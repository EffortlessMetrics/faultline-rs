use faultline_ports::RunStorePort;
use faultline_types::{
    AnalysisReport, AnalysisRequest, FaultlineError, ProbeObservation, Result, RunHandle,
};
use std::fs;
use std::path::{Path, PathBuf};

/// Write `content` to `{target}.tmp` then atomically rename to `target`.
/// This prevents readers from seeing a partially-written file if the process
/// is interrupted mid-write.
fn atomic_write(target: &Path, content: &[u8]) -> std::io::Result<()> {
    let tmp = target.with_extension("tmp");
    fs::write(&tmp, content)?;
    fs::rename(&tmp, target)?;
    Ok(())
}

/// Check if a process with the given PID is alive.
///
/// This is a local-machine-only, best-effort check — not a distributed lock.
/// On Unix, uses `kill(pid, 0)` to check process existence.
/// On non-Unix platforms, always returns `false` (treats lock as stale).
fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // PIDs must be positive; negative or zero values have special meaning
        // in kill() (process groups) and must not be used for liveness checks.
        let pid_i32 = pid as libc::pid_t;
        if pid_i32 <= 0 {
            return false;
        }
        // SAFETY: kill with signal 0 does not send a signal — it only checks
        // whether the process exists and we have permission to signal it.
        // Returns 0 if we can signal, or -1 with:
        //   ESRCH  — process does not exist (dead)
        //   EPERM  — process exists but we lack permission (alive)
        let ret = unsafe { libc::kill(pid_i32, 0) };
        if ret == 0 {
            return true;
        }
        // EPERM means the process exists but we can't signal it — still alive
        std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

/// Acquire a lock file at `lock_path`. If a lock already exists and is held by
/// a live process, returns an error. If the lock is stale (dead PID), removes
/// it and re-acquires.
///
/// The lock file contains `{pid}\n{timestamp}` and serves as a local-machine-only,
/// best-effort single-writer guard. It is NOT a distributed lock.
fn acquire_lock(lock_path: &Path) -> Result<()> {
    let current_pid = std::process::id();
    if lock_path.exists() {
        let content = fs::read_to_string(lock_path)?;
        if let Some(pid_str) = content.lines().next()
            && let Ok(pid) = pid_str.trim().parse::<u32>()
        {
            if pid == current_pid {
                // Same process re-acquiring — allow it (e.g., resume)
            } else if is_process_alive(pid) {
                return Err(FaultlineError::Store(format!(
                    "run locked by process {}",
                    pid
                )));
            }
        }
        // Stale lock (dead PID or unparseable) or same-process re-entry — remove it
        fs::remove_file(lock_path)?;
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    fs::write(lock_path, format!("{}\n{}", current_pid, timestamp))?;
    Ok(())
}

/// Release the lock file by deleting it. Ignores errors if the file doesn't exist.
fn release_lock(lock_path: &Path) {
    let _ = fs::remove_file(lock_path);
}

#[derive(Debug, Clone)]
pub struct FileRunStore {
    root: PathBuf,
}

impl FileRunStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn run_root(&self, run_id: &str) -> PathBuf {
        self.root.join(run_id)
    }

    fn lock_path(&self, run: &RunHandle) -> PathBuf {
        run.root.join(".lock")
    }

    fn observations_path(&self, run: &RunHandle) -> PathBuf {
        run.root.join("observations.json")
    }

    fn request_path(&self, run: &RunHandle) -> PathBuf {
        run.root.join("request.json")
    }

    fn report_path(&self, run: &RunHandle) -> PathBuf {
        run.root.join("report.json")
    }

    fn read_json_or_default<T>(&self, path: &Path) -> Result<T>
    where
        T: serde::de::DeserializeOwned + Default,
    {
        if !path.exists() {
            return Ok(T::default());
        }
        let raw = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    /// Save full probe stdout/stderr to per-commit log files in `{run_dir}/logs/`.
    ///
    /// Called by the app layer when truncated output is detected (ends with
    /// `"[truncated]"`). Full-log persistence is a store concern, not a
    /// probe-exec concern.
    pub fn save_probe_logs(
        &self,
        run: &RunHandle,
        commit_sha: &str,
        stdout: &str,
        stderr: &str,
    ) -> Result<()> {
        let logs_dir = run.root.join("logs");
        fs::create_dir_all(&logs_dir)?;
        atomic_write(
            &logs_dir.join(format!("{}_stdout.log", commit_sha)),
            stdout.as_bytes(),
        )?;
        atomic_write(
            &logs_dir.join(format!("{}_stderr.log", commit_sha)),
            stderr.as_bytes(),
        )?;
        Ok(())
    }
}

impl RunStorePort for FileRunStore {
    fn prepare_run(&self, request: &AnalysisRequest) -> Result<RunHandle> {
        let run_id = request.fingerprint();
        let root = self.run_root(&run_id);
        let resumed = root.exists();
        fs::create_dir_all(&root)?;
        let handle = RunHandle {
            id: run_id,
            root,
            resumed,
            schema_version: "0.1.0".into(),
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
        };
        acquire_lock(&self.lock_path(&handle))?;
        atomic_write(
            &self.request_path(&handle),
            serde_json::to_string_pretty(request)?.as_bytes(),
        )?;
        let metadata = serde_json::json!({
            "schema_version": &handle.schema_version,
            "tool_version": &handle.tool_version,
        });
        atomic_write(
            &handle.root.join("metadata.json"),
            serde_json::to_string_pretty(&metadata)?.as_bytes(),
        )?;
        Ok(handle)
    }

    fn load_observations(&self, run: &RunHandle) -> Result<Vec<ProbeObservation>> {
        self.read_json_or_default(&self.observations_path(run))
    }

    fn save_observation(&self, run: &RunHandle, observation: &ProbeObservation) -> Result<()> {
        let mut observations: Vec<ProbeObservation> = self.load_observations(run)?;
        if let Some(existing) = observations
            .iter_mut()
            .find(|item| item.commit == observation.commit)
        {
            *existing = observation.clone();
        } else {
            observations.push(observation.clone());
        }
        observations.sort_by(|a, b| a.sequence_index.cmp(&b.sequence_index));
        atomic_write(
            &self.observations_path(run),
            serde_json::to_string_pretty(&observations)?.as_bytes(),
        )?;
        Ok(())
    }

    fn save_report(&self, run: &RunHandle, report: &AnalysisReport) -> Result<()> {
        atomic_write(
            &self.report_path(run),
            serde_json::to_string_pretty(report)?.as_bytes(),
        )?;
        release_lock(&self.lock_path(run));
        Ok(())
    }

    fn load_report(&self, run: &RunHandle) -> Result<Option<AnalysisReport>> {
        let path = self.report_path(run);
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)?;
        Ok(Some(serde_json::from_str(&raw)?))
    }

    fn save_probe_logs(
        &self,
        run: &RunHandle,
        commit_sha: &str,
        stdout: &str,
        stderr: &str,
    ) -> Result<()> {
        self.save_probe_logs(run, commit_sha, stdout, stderr)
    }

    fn clear_observations(&self, run: &RunHandle) -> Result<()> {
        let path = self.observations_path(run);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    fn delete_run(&self, run: &RunHandle) -> Result<()> {
        if run.root.exists() {
            fs::remove_dir_all(&run.root)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use faultline_codes::{AmbiguityReason, ObservationClass, ProbeKind};
    use faultline_types::{
        AnalysisReport, AnalysisRequest, ChangeStatus, CommitId, Confidence, FlakePolicy,
        HistoryMode, LocalizationOutcome, PathChange, ProbeObservation, ProbeSpec,
        RevisionSequence, RevisionSpec, SearchPolicy, ShellKind, SubsystemBucket, SurfaceSummary,
    };
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn sample_probe_spec() -> ProbeSpec {
        ProbeSpec::Exec {
            kind: ProbeKind::Test,
            program: "cargo".into(),
            args: vec!["test".into()],
            env: vec![],
            timeout_seconds: 300,
        }
    }

    fn sample_request() -> AnalysisRequest {
        AnalysisRequest {
            repo_root: PathBuf::from("/tmp/repo"),
            good: RevisionSpec("abc123".into()),
            bad: RevisionSpec("def456".into()),
            history_mode: HistoryMode::AncestryPath,
            probe: sample_probe_spec(),
            policy: SearchPolicy::default(),
        }
    }

    fn sample_observation(commit: &str, class: ObservationClass) -> ProbeObservation {
        ProbeObservation {
            commit: CommitId(commit.into()),
            class,
            kind: ProbeKind::Test,
            exit_code: Some(0),
            timed_out: false,
            duration_ms: 100,
            stdout: "ok".into(),
            stderr: String::new(),
            sequence_index: 0,
            signal_number: None,
            probe_command: String::new(),
            working_dir: String::new(),
            flake_signal: None,
        }
    }

    fn sample_report() -> AnalysisReport {
        AnalysisReport {
            schema_version: "0.1.0".into(),
            run_id: "run-1".into(),
            created_at_epoch_seconds: 1700000000,
            request: sample_request(),
            sequence: RevisionSequence {
                revisions: vec![CommitId("abc123".into()), CommitId("def456".into())],
            },
            observations: vec![sample_observation("abc123", ObservationClass::Pass)],
            outcome: LocalizationOutcome::FirstBad {
                last_good: CommitId("abc123".into()),
                first_bad: CommitId("def456".into()),
                confidence: Confidence::high(),
            },
            changed_paths: vec![PathChange {
                status: ChangeStatus::Modified,
                path: "src/main.rs".into(),
            }],
            surface: SurfaceSummary {
                total_changes: 1,
                buckets: vec![SubsystemBucket {
                    name: "src".into(),
                    change_count: 1,
                    paths: vec!["src/main.rs".into()],
                    surface_kinds: vec!["source".into()],
                }],
                execution_surfaces: vec![],
            },
            suspect_surface: vec![],
            reproduction_capsules: vec![],
        }
    }

    // --- prepare_run tests ---

    #[test]
    fn prepare_run_creates_directory_and_request_json() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();

        let handle = store.prepare_run(&request).unwrap();

        assert!(!handle.resumed);
        assert!(handle.root.exists());
        assert_eq!(handle.id, request.fingerprint());

        let request_path = handle.root.join("request.json");
        assert!(request_path.exists());
        let raw = fs::read_to_string(&request_path).unwrap();
        let deserialized: AnalysisRequest = serde_json::from_str(&raw).unwrap();
        assert_eq!(deserialized, request);
    }

    #[test]
    fn prepare_run_sets_resumed_on_second_call() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();

        let handle1 = store.prepare_run(&request).unwrap();
        assert!(!handle1.resumed);

        let handle2 = store.prepare_run(&request).unwrap();
        assert!(handle2.resumed);
        assert_eq!(handle1.id, handle2.id);
    }

    // --- load_observations tests ---

    #[test]
    fn load_observations_returns_empty_when_no_file() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        let obs = store.load_observations(&handle).unwrap();
        assert!(obs.is_empty());
    }

    // --- save_observation tests ---

    #[test]
    fn save_and_load_single_observation() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        let obs = sample_observation("abc123", ObservationClass::Pass);
        store.save_observation(&handle, &obs).unwrap();

        let loaded = store.load_observations(&handle).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0], obs);
    }

    #[test]
    fn save_observation_upserts_by_commit_id() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        let obs1 = sample_observation("abc123", ObservationClass::Pass);
        store.save_observation(&handle, &obs1).unwrap();

        // Save again with different class for same commit — should replace
        let obs2 = sample_observation("abc123", ObservationClass::Fail);
        store.save_observation(&handle, &obs2).unwrap();

        let loaded = store.load_observations(&handle).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].class, ObservationClass::Fail);
    }

    #[test]
    fn save_observation_sorts_by_sequence_index() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        // Save in non-sequential order with explicit sequence indices
        let mut obs_c = sample_observation("ccc", ObservationClass::Pass);
        obs_c.sequence_index = 2;
        let mut obs_a = sample_observation("aaa", ObservationClass::Fail);
        obs_a.sequence_index = 0;
        let mut obs_b = sample_observation("bbb", ObservationClass::Skip);
        obs_b.sequence_index = 1;

        store.save_observation(&handle, &obs_c).unwrap();
        store.save_observation(&handle, &obs_a).unwrap();
        store.save_observation(&handle, &obs_b).unwrap();

        let loaded = store.load_observations(&handle).unwrap();
        assert_eq!(loaded.len(), 3);
        // Sorted by sequence_index, not by commit hash
        assert_eq!(loaded[0].commit.0, "aaa");
        assert_eq!(loaded[0].sequence_index, 0);
        assert_eq!(loaded[1].commit.0, "bbb");
        assert_eq!(loaded[1].sequence_index, 1);
        assert_eq!(loaded[2].commit.0, "ccc");
        assert_eq!(loaded[2].sequence_index, 2);
    }

    // --- save_report tests ---

    #[test]
    fn save_and_read_report() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        let report = sample_report();
        store.save_report(&handle, &report).unwrap();

        let report_path = handle.root.join("report.json");
        assert!(report_path.exists());
        let raw = fs::read_to_string(&report_path).unwrap();
        let deserialized: AnalysisReport = serde_json::from_str(&raw).unwrap();
        assert_eq!(deserialized, report);
    }

    // --- load_report tests ---

    #[test]
    fn load_report_returns_none_when_no_file() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        let loaded = store.load_report(&handle).unwrap();
        assert!(
            loaded.is_none(),
            "load_report should return None when report.json does not exist"
        );
    }

    #[test]
    fn load_report_returns_saved_report() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        let report = sample_report();
        store.save_report(&handle, &report).unwrap();

        let loaded = store.load_report(&handle).unwrap();
        assert_eq!(loaded, Some(report));
    }

    // --- FileRunStore::new tests ---

    #[test]
    fn new_creates_root_directory() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("deep").join("nested").join("runs");
        assert!(!root.exists());

        let _store = FileRunStore::new(&root).unwrap();
        assert!(root.exists());
    }

    // --- Lock file tests ---

    #[test]
    fn prepare_run_creates_lock_file() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();

        let handle = store.prepare_run(&request).unwrap();

        let lock_path = handle.root.join(".lock");
        assert!(
            lock_path.exists(),
            "lock file should be created by prepare_run"
        );

        let content = fs::read_to_string(&lock_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "lock file should contain PID and timestamp");

        let pid: u32 = lines[0].parse().expect("first line should be a PID");
        assert_eq!(
            pid,
            std::process::id(),
            "lock PID should match current process"
        );

        let _timestamp: u64 = lines[1].parse().expect("second line should be a timestamp");
    }

    #[test]
    fn prepare_run_allows_same_process_reentry() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();

        let handle1 = store.prepare_run(&request).unwrap();
        assert!(handle1.root.join(".lock").exists());

        // Same process calling prepare_run again should succeed (re-entry)
        let handle2 = store.prepare_run(&request).unwrap();
        assert!(handle2.root.join(".lock").exists());
        assert_eq!(handle1.id, handle2.id);
    }

    #[test]
    #[cfg(unix)]
    fn prepare_run_rejects_lock_held_by_another_live_process() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();

        // Create the run directory and write a lock file with PID 1 (init/launchd — always alive on Unix)
        let run_id = request.fingerprint();
        let run_dir = tmp.path().join("runs").join(&run_id);
        fs::create_dir_all(&run_dir).unwrap();
        let lock_path = run_dir.join(".lock");
        fs::write(&lock_path, "1\n1700000000").unwrap();

        let result = store.prepare_run(&request);
        assert!(
            result.is_err(),
            "should fail when lock is held by another live process"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("run locked by process 1"),
            "error should mention the locking PID, got: {}",
            err_msg
        );
    }

    #[test]
    #[cfg(not(unix))]
    fn prepare_run_treats_foreign_pid_lock_as_stale_on_non_unix() {
        // On non-Unix platforms, PID liveness checks always return false,
        // so any foreign-PID lock is treated as stale and cleaned up.
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();

        let run_id = request.fingerprint();
        let run_dir = tmp.path().join("runs").join(&run_id);
        fs::create_dir_all(&run_dir).unwrap();
        let lock_path = run_dir.join(".lock");
        fs::write(&lock_path, "1\n1700000000").unwrap();

        // Should succeed — on non-Unix, all foreign locks are treated as stale
        let handle = store.prepare_run(&request).unwrap();
        let content = fs::read_to_string(handle.root.join(".lock")).unwrap();
        let pid: u32 = content.lines().next().unwrap().parse().unwrap();
        assert_eq!(pid, std::process::id());
    }

    #[test]
    fn prepare_run_cleans_stale_lock_from_dead_process() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();

        // Create the run directory and write a lock file with a very high PID
        // that is almost certainly not alive
        let run_id = request.fingerprint();
        let run_dir = tmp.path().join("runs").join(&run_id);
        fs::create_dir_all(&run_dir).unwrap();
        let lock_path = run_dir.join(".lock");
        fs::write(&lock_path, "4294967295\n1700000000").unwrap();

        // Should succeed — stale lock is cleaned up
        let handle = store.prepare_run(&request).unwrap();
        assert!(handle.root.join(".lock").exists());

        // Verify the lock now contains our PID
        let content = fs::read_to_string(handle.root.join(".lock")).unwrap();
        let pid: u32 = content.lines().next().unwrap().parse().unwrap();
        assert_eq!(pid, std::process::id());
    }

    #[test]
    fn save_report_releases_lock() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();

        let handle = store.prepare_run(&request).unwrap();
        assert!(
            handle.root.join(".lock").exists(),
            "lock should exist after prepare_run"
        );

        let report = sample_report();
        store.save_report(&handle, &report).unwrap();
        assert!(
            !handle.root.join(".lock").exists(),
            "lock should be released after save_report"
        );
    }

    // --- atomic_write tests ---

    #[test]
    fn atomic_write_produces_correct_content_and_no_tmp_file() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("data.json");

        let content = b"{\"key\": \"value\", \"number\": 42}";
        atomic_write(&target, content).unwrap();

        // Verify the target file has the correct content
        let read_back = fs::read(&target).unwrap();
        assert_eq!(
            read_back, content,
            "atomic_write must produce correct file content"
        );

        // Verify no .tmp file is left behind
        let tmp_path = target.with_extension("tmp");
        assert!(
            !tmp_path.exists(),
            "atomic_write must not leave a .tmp file behind after success"
        );
    }

    // --- save_probe_logs tests ---

    #[test]
    fn save_probe_logs_creates_log_files() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        store
            .save_probe_logs(
                &handle,
                "abc123",
                "full stdout output",
                "full stderr output",
            )
            .unwrap();

        let stdout_path = handle.root.join("logs").join("abc123_stdout.log");
        let stderr_path = handle.root.join("logs").join("abc123_stderr.log");
        assert!(stdout_path.exists(), "stdout log file should exist");
        assert!(stderr_path.exists(), "stderr log file should exist");
        assert_eq!(
            fs::read_to_string(&stdout_path).unwrap(),
            "full stdout output"
        );
        assert_eq!(
            fs::read_to_string(&stderr_path).unwrap(),
            "full stderr output"
        );
    }

    #[test]
    fn save_probe_logs_creates_logs_directory() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        let logs_dir = handle.root.join("logs");
        assert!(
            !logs_dir.exists(),
            "logs dir should not exist before save_probe_logs"
        );

        store
            .save_probe_logs(&handle, "abc123", "out", "err")
            .unwrap();
        assert!(
            logs_dir.exists(),
            "logs dir should be created by save_probe_logs"
        );
    }

    #[test]
    fn save_probe_logs_overwrites_existing_logs() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        store
            .save_probe_logs(&handle, "abc123", "first", "first")
            .unwrap();
        store
            .save_probe_logs(&handle, "abc123", "second", "second")
            .unwrap();

        let stdout_path = handle.root.join("logs").join("abc123_stdout.log");
        assert_eq!(fs::read_to_string(&stdout_path).unwrap(), "second");
    }

    #[test]
    fn save_probe_logs_handles_multiple_commits() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        store
            .save_probe_logs(&handle, "aaa111", "out-a", "err-a")
            .unwrap();
        store
            .save_probe_logs(&handle, "bbb222", "out-b", "err-b")
            .unwrap();

        assert_eq!(
            fs::read_to_string(handle.root.join("logs").join("aaa111_stdout.log")).unwrap(),
            "out-a"
        );
        assert_eq!(
            fs::read_to_string(handle.root.join("logs").join("bbb222_stderr.log")).unwrap(),
            "err-b"
        );
    }

    // --- Property tests: Property 11 — Run Store Round-Trip ---

    use proptest::prelude::*;

    fn arb_probe_kind() -> impl Strategy<Value = ProbeKind> {
        prop_oneof![
            Just(ProbeKind::Build),
            Just(ProbeKind::Test),
            Just(ProbeKind::Lint),
            Just(ProbeKind::PerfThreshold),
            Just(ProbeKind::Custom),
        ]
    }

    fn arb_observation_class() -> impl Strategy<Value = ObservationClass> {
        prop_oneof![
            Just(ObservationClass::Pass),
            Just(ObservationClass::Fail),
            Just(ObservationClass::Skip),
            Just(ObservationClass::Indeterminate),
        ]
    }

    fn arb_commit_id() -> impl Strategy<Value = CommitId> {
        "[a-f0-9]{8,40}".prop_map(CommitId)
    }

    fn arb_probe_observation() -> impl Strategy<Value = ProbeObservation> {
        (
            arb_commit_id(),
            arb_observation_class(),
            arb_probe_kind(),
            prop::option::of(any::<i32>()),
            any::<bool>(),
            any::<u64>(),
            "[a-z ]{0,20}",
            "[a-z ]{0,20}",
            any::<u64>(),
            prop::option::of(any::<i32>()),
            "[a-z/ ]{0,30}",
            "[a-z/ ]{0,30}",
        )
            .prop_map(
                |(
                    commit,
                    class,
                    kind,
                    exit_code,
                    timed_out,
                    duration_ms,
                    stdout,
                    stderr,
                    sequence_index,
                    signal_number,
                    probe_command,
                    working_dir,
                )| {
                    ProbeObservation {
                        commit,
                        class,
                        kind,
                        exit_code,
                        timed_out,
                        duration_ms,
                        stdout,
                        stderr,
                        sequence_index,
                        signal_number,
                        probe_command,
                        working_dir,
                        flake_signal: None,
                    }
                },
            )
    }

    fn arb_revision_spec() -> impl Strategy<Value = RevisionSpec> {
        "[a-f0-9]{8,40}".prop_map(RevisionSpec)
    }

    fn arb_history_mode() -> impl Strategy<Value = HistoryMode> {
        prop_oneof![
            Just(HistoryMode::AncestryPath),
            Just(HistoryMode::FirstParent),
        ]
    }

    fn arb_shell_kind() -> impl Strategy<Value = ShellKind> {
        prop_oneof![
            Just(ShellKind::Default),
            Just(ShellKind::PosixSh),
            Just(ShellKind::Cmd),
            Just(ShellKind::PowerShell),
        ]
    }

    fn arb_probe_spec() -> impl Strategy<Value = ProbeSpec> {
        prop_oneof![
            (
                arb_probe_kind(),
                "[a-z]{1,10}",
                prop::collection::vec("[a-z0-9]{1,8}", 0..3),
                prop::collection::vec(("[A-Z]{1,4}", "[a-z0-9]{1,6}"), 0..2),
                1u64..600,
            )
                .prop_map(|(kind, program, args, env, timeout_seconds)| {
                    ProbeSpec::Exec {
                        kind,
                        program,
                        args,
                        env,
                        timeout_seconds,
                    }
                }),
            (
                arb_probe_kind(),
                arb_shell_kind(),
                "[a-z ]{1,20}",
                1u64..600
            )
                .prop_map(|(kind, shell, script, timeout_seconds)| {
                    ProbeSpec::Shell {
                        kind,
                        shell,
                        script,
                        env: vec![],
                        timeout_seconds,
                    }
                }),
        ]
    }

    fn arb_search_policy() -> impl Strategy<Value = SearchPolicy> {
        (1usize..128).prop_map(|max_probes| SearchPolicy {
            max_probes,
            flake_policy: FlakePolicy::default(),
        })
    }

    fn arb_analysis_request() -> impl Strategy<Value = AnalysisRequest> {
        (
            "[a-z/]{1,20}",
            arb_revision_spec(),
            arb_revision_spec(),
            arb_history_mode(),
            arb_probe_spec(),
            arb_search_policy(),
        )
            .prop_map(|(repo_root, good, bad, history_mode, probe, policy)| {
                AnalysisRequest {
                    repo_root: PathBuf::from(repo_root),
                    good,
                    bad,
                    history_mode,
                    probe,
                    policy,
                }
            })
    }

    fn arb_confidence() -> impl Strategy<Value = Confidence> {
        (any::<u8>(), "[a-z]{1,10}").prop_map(|(score, label)| Confidence { score, label })
    }

    fn arb_ambiguity_reason() -> impl Strategy<Value = AmbiguityReason> {
        prop_oneof![
            Just(AmbiguityReason::MissingPassBoundary),
            Just(AmbiguityReason::MissingFailBoundary),
            Just(AmbiguityReason::NonMonotonicEvidence),
            Just(AmbiguityReason::SkippedRevision),
            Just(AmbiguityReason::IndeterminateRevision),
            Just(AmbiguityReason::UntestableWindow),
            Just(AmbiguityReason::BoundaryValidationFailed),
            Just(AmbiguityReason::NeedsMoreProbes),
            Just(AmbiguityReason::MaxProbesExhausted),
        ]
    }

    fn arb_localization_outcome() -> impl Strategy<Value = LocalizationOutcome> {
        prop_oneof![
            (arb_commit_id(), arb_commit_id(), arb_confidence()).prop_map(
                |(last_good, first_bad, confidence)| {
                    LocalizationOutcome::FirstBad {
                        last_good,
                        first_bad,
                        confidence,
                    }
                }
            ),
            (
                arb_commit_id(),
                arb_commit_id(),
                arb_confidence(),
                prop::collection::vec(arb_ambiguity_reason(), 1..4),
            )
                .prop_map(
                    |(lower_bound_exclusive, upper_bound_inclusive, confidence, reasons)| {
                        LocalizationOutcome::SuspectWindow {
                            lower_bound_exclusive,
                            upper_bound_inclusive,
                            confidence,
                            reasons,
                        }
                    }
                ),
            prop::collection::vec(arb_ambiguity_reason(), 1..4)
                .prop_map(|reasons| LocalizationOutcome::Inconclusive { reasons }),
        ]
    }

    fn arb_change_status() -> impl Strategy<Value = ChangeStatus> {
        prop_oneof![
            Just(ChangeStatus::Added),
            Just(ChangeStatus::Modified),
            Just(ChangeStatus::Deleted),
            Just(ChangeStatus::Renamed),
            Just(ChangeStatus::TypeChanged),
            Just(ChangeStatus::Unknown),
        ]
    }

    fn arb_path_change() -> impl Strategy<Value = PathChange> {
        (arb_change_status(), "[a-z/]{1,30}").prop_map(|(status, path)| PathChange { status, path })
    }

    fn arb_subsystem_bucket() -> impl Strategy<Value = SubsystemBucket> {
        (
            "[a-z]{1,10}",
            0usize..20,
            prop::collection::vec("[a-z/]{1,20}", 0..5),
            prop::collection::vec("[a-z]{1,10}", 0..3),
        )
            .prop_map(
                |(name, change_count, paths, surface_kinds)| SubsystemBucket {
                    name,
                    change_count,
                    paths,
                    surface_kinds,
                },
            )
    }

    fn arb_surface_summary() -> impl Strategy<Value = SurfaceSummary> {
        (
            0usize..50,
            prop::collection::vec(arb_subsystem_bucket(), 0..5),
            prop::collection::vec("[a-z/]{1,20}", 0..3),
        )
            .prop_map(
                |(total_changes, buckets, execution_surfaces)| SurfaceSummary {
                    total_changes,
                    buckets,
                    execution_surfaces,
                },
            )
    }

    fn arb_revision_sequence() -> impl Strategy<Value = RevisionSequence> {
        prop::collection::vec(arb_commit_id(), 2..10)
            .prop_map(|revisions| RevisionSequence { revisions })
    }

    fn arb_analysis_report() -> impl Strategy<Value = AnalysisReport> {
        (
            "[a-z0-9-]{1,20}",
            any::<u64>(),
            arb_analysis_request(),
            arb_revision_sequence(),
            prop::collection::vec(arb_probe_observation(), 0..5),
            arb_localization_outcome(),
            prop::collection::vec(arb_path_change(), 0..5),
            arb_surface_summary(),
        )
            .prop_map(
                |(
                    run_id,
                    created_at_epoch_seconds,
                    request,
                    sequence,
                    observations,
                    outcome,
                    changed_paths,
                    surface,
                )| {
                    AnalysisReport {
                        schema_version: "0.1.0".into(),
                        run_id,
                        created_at_epoch_seconds,
                        request,
                        sequence,
                        observations,
                        outcome,
                        changed_paths,
                        surface,
                        suspect_surface: vec![],
                        reproduction_capsules: vec![],
                    }
                },
            )
    }

    fn arb_analysis_report_with_custom_schema_version() -> impl Strategy<Value = AnalysisReport> {
        ("[a-z0-9\\.]{1,15}", arb_analysis_report()).prop_map(|(schema_version, mut report)| {
            report.schema_version = schema_version;
            report
        })
    }

    // Feature: v01-release-train, Property 11: Run Store Round-Trip
    // **Validates: Requirements 4.2, 4.5, 4.6**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_observation_round_trip(obs in arb_probe_observation()) {
            let tmp = TempDir::new().unwrap();
            let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
            let request = sample_request();
            let handle = store.prepare_run(&request).unwrap();

            store.save_observation(&handle, &obs).unwrap();
            let loaded = store.load_observations(&handle).unwrap();

            prop_assert!(
                loaded.iter().any(|o| *o == obs),
                "saved ProbeObservation must be present in loaded observations"
            );
        }

        #[test]
        fn prop_report_round_trip(report in arb_analysis_report()) {
            let tmp = TempDir::new().unwrap();
            let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
            let request = sample_request();
            let handle = store.prepare_run(&request).unwrap();

            store.save_report(&handle, &report).unwrap();

            let report_path = handle.root.join("report.json");
            let raw = fs::read_to_string(&report_path).unwrap();
            let deserialized: AnalysisReport = serde_json::from_str(&raw).unwrap();
            prop_assert_eq!(report, deserialized, "AnalysisReport round-trip must preserve equality");
        }

        #[test]
        fn prop_request_round_trip(request in arb_analysis_request()) {
            let tmp = TempDir::new().unwrap();
            let store = FileRunStore::new(tmp.path().join("runs")).unwrap();

            let handle = store.prepare_run(&request).unwrap();

            let request_path = handle.root.join("request.json");
            let raw = fs::read_to_string(&request_path).unwrap();
            let deserialized: AnalysisRequest = serde_json::from_str(&raw).unwrap();
            prop_assert_eq!(request, deserialized, "AnalysisRequest round-trip must preserve equality");
        }

        // Feature: v01-hardening, Property 29: Store Observation Sequence Order
        // **Validates: Requirements 3.3, 6.5**
        #[test]
        fn prop_store_observation_sequence_order(
            observations in prop::collection::vec(arb_probe_observation(), 2..=10)
                .prop_map(|mut obs| {
                    // Assign distinct sequence_index values and distinct commit IDs
                    // so upsert doesn't collapse entries.
                    for (i, o) in obs.iter_mut().enumerate() {
                        o.sequence_index = i as u64 * 3 + 1; // distinct, non-contiguous
                        o.commit = CommitId(format!("commit_{:04}", i));
                    }
                    obs
                })
                .prop_shuffle()
        ) {
            let tmp = TempDir::new().unwrap();
            let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
            let request = sample_request();
            let handle = store.prepare_run(&request).unwrap();

            // Save observations in shuffled order
            for obs in &observations {
                store.save_observation(&handle, obs).unwrap();
            }

            // Load and verify ascending sequence_index order
            let loaded = store.load_observations(&handle).unwrap();
            prop_assert_eq!(
                loaded.len(),
                observations.len(),
                "loaded count must match saved count"
            );

            for window in loaded.windows(2) {
                prop_assert!(
                    window[0].sequence_index < window[1].sequence_index,
                    "observations must be in ascending sequence_index order, got {} >= {}",
                    window[0].sequence_index,
                    window[1].sequence_index
                );
            }
        }

        // Feature: v01-release-train, Property 12: Run Store Resumability
        // **Validates: Requirements 4.3**
        #[test]
        fn prop_run_store_resumability(
            request in arb_analysis_request(),
            observations in prop::collection::vec(arb_probe_observation(), 1..=5),
        ) {
            let tmp = TempDir::new().unwrap();
            let store = FileRunStore::new(tmp.path().join("runs")).unwrap();

            // First prepare_run — should NOT be resumed
            let handle1 = store.prepare_run(&request).unwrap();
            prop_assert!(!handle1.resumed, "first prepare_run must have resumed == false");

            // Save all observations via the first handle
            for obs in &observations {
                store.save_observation(&handle1, obs).unwrap();
            }

            // Second prepare_run — should be resumed
            let handle2 = store.prepare_run(&request).unwrap();
            prop_assert!(handle2.resumed, "second prepare_run must have resumed == true");
            prop_assert_eq!(&handle1.id, &handle2.id, "both handles must share the same run ID");

            // Load observations from the second handle
            let loaded = store.load_observations(&handle2).unwrap();

            // Verify all saved observations are present in the loaded list.
            // Note: save_observation upserts by commit ID, so if multiple observations
            // share the same commit, only the last one is kept.
            let mut expected_by_commit: std::collections::HashMap<&str, &ProbeObservation> =
                std::collections::HashMap::new();
            for obs in &observations {
                expected_by_commit.insert(&obs.commit.0, obs);
            }

            for (commit_id, expected_obs) in &expected_by_commit {
                prop_assert!(
                    loaded.iter().any(|o| o == *expected_obs),
                    "observation for commit {} must be present in loaded observations after resume",
                    commit_id
                );
            }

            prop_assert_eq!(
                loaded.len(),
                expected_by_commit.len(),
                "loaded observation count must match unique commit count"
            );
        }

        // Feature: v01-hardening, Property 31: Report Load Round-Trip
        // **Validates: Requirement 6.11**
        #[test]
        fn prop_report_load_round_trip(report in arb_analysis_report()) {
            let tmp = TempDir::new().unwrap();
            let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
            let request = sample_request();
            let handle = store.prepare_run(&request).unwrap();

            store.save_report(&handle, &report).unwrap();
            let loaded = store.load_report(&handle).unwrap();

            prop_assert_eq!(
                loaded,
                Some(report),
                "load_report after save_report must return Some(original report)"
            );
        }

        // Feature: v01-hardening, Property 24: Schema Version Round-Trip
        // **Validates: Requirements 1.4, 1.6**
        #[test]
        fn prop_schema_version_round_trip(
            report in arb_analysis_report_with_custom_schema_version()
        ) {
            let tmp = TempDir::new().unwrap();
            let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
            let request = sample_request();
            let handle = store.prepare_run(&request).unwrap();

            let original_version = report.schema_version.clone();

            // Verify JSON serialization round-trip preserves schema_version
            let json = serde_json::to_string_pretty(&report).unwrap();
            let deserialized: AnalysisReport = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(
                &deserialized.schema_version,
                &original_version,
                "JSON round-trip must preserve schema_version"
            );

            // Verify store save/load round-trip preserves schema_version
            store.save_report(&handle, &report).unwrap();
            let loaded = store.load_report(&handle).unwrap();
            prop_assert!(loaded.is_some(), "load_report must return Some after save_report");
            prop_assert_eq!(
                &loaded.unwrap().schema_version,
                &original_version,
                "store round-trip must preserve schema_version"
            );
        }

        // Feature: v01-hardening, Property 30: Version Metadata Persistence
        // **Validates: Requirement 6.4**
        #[test]
        fn prop_version_metadata_persistence(request in arb_analysis_request()) {
            let tmp = TempDir::new().unwrap();
            let store = FileRunStore::new(tmp.path().join("runs")).unwrap();

            let handle = store.prepare_run(&request).unwrap();

            // Read metadata.json from the run directory
            let metadata_path = handle.root.join("metadata.json");
            prop_assert!(metadata_path.exists(), "metadata.json must exist after prepare_run");

            let raw = fs::read_to_string(&metadata_path).unwrap();
            let metadata: serde_json::Value = serde_json::from_str(&raw).unwrap();

            prop_assert_eq!(
                metadata["schema_version"].as_str().unwrap(),
                "0.1.0",
                "schema_version in metadata.json must be 0.1.0"
            );

            prop_assert_eq!(
                metadata["tool_version"].as_str().unwrap(),
                env!("CARGO_PKG_VERSION"),
                "tool_version in metadata.json must match workspace version"
            );
        }
    }
}
