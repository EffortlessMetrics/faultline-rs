use faultline_codes::ObservationClass;
use faultline_ports::ProbePort;
use faultline_types::{
    CheckedOutRevision, FaultlineError, ProbeObservation, ProbeSpec, Result, ShellKind,
};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_TRUNCATION_LIMIT: usize = 64 * 1024; // 64 KiB
const TRUNCATION_MARKER: &str = "[truncated]";

/// Truncate output to at most `limit` bytes and append a marker when it exceeds the limit.
/// The truncation point backs up to a UTF-8 boundary rather than splitting a codepoint.
/// `ProbeObservation` stores this bounded preview; it is not a complete retained process log.
fn truncate_output(output: &str, limit: usize) -> String {
    if output.len() <= limit {
        return output.to_string();
    }

    let mut end = limit.min(output.len());
    while end > 0 && !output.is_char_boundary(end) {
        end -= 1;
    }

    let mut truncated = output[..end].to_string();
    truncated.push_str(TRUNCATION_MARKER);
    truncated
}

fn read_stream<R: Read>(mut stream: R) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn spawn_stream_reader<R>(
    stream: R,
    stream_name: &'static str,
) -> Result<thread::JoinHandle<std::io::Result<Vec<u8>>>>
where
    R: Read + Send + 'static,
{
    thread::Builder::new()
        .name(format!("faultline-{stream_name}-reader"))
        .spawn(move || read_stream(stream))
        .map_err(|err| {
            FaultlineError::Probe(format!("failed to start probe {stream_name} reader: {err}"))
        })
}

fn terminate_after_reader_start_failure(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn join_stream_reader(
    reader: thread::JoinHandle<std::io::Result<Vec<u8>>>,
    stream_name: &str,
) -> Result<Vec<u8>> {
    match reader.join() {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(err)) => Err(FaultlineError::Probe(format!(
            "failed to read probe {stream_name}: {err}"
        ))),
        Err(_) => Err(FaultlineError::Probe(format!(
            "probe {stream_name} reader thread panicked"
        ))),
    }
}

#[derive(Debug, Default, Clone)]
pub struct ExecProbeAdapter;

impl ExecProbeAdapter {
    /// Run a probe with a custom truncation limit. Used internally and for testing.
    fn run_with_limit(
        &self,
        checkout: &CheckedOutRevision,
        probe: &ProbeSpec,
        truncation_limit: usize,
    ) -> Result<ProbeObservation> {
        let kind = probe.kind();
        let timeout = Duration::from_secs(probe.timeout_seconds());
        let mut command = build_command(probe);
        command
            .current_dir(&checkout.path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let started = Instant::now();
        let mut child = command
            .spawn()
            .map_err(|err| FaultlineError::Probe(err.to_string()))?;

        // Drain both pipes while the child is running. Waiting for the child before
        // reading can deadlock once either operating-system pipe buffer fills.
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| FaultlineError::Probe("failed to capture probe stdout".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| FaultlineError::Probe("failed to capture probe stderr".to_string()))?;
        let stdout_reader = match spawn_stream_reader(stdout, "stdout") {
            Ok(reader) => reader,
            Err(err) => {
                terminate_after_reader_start_failure(&mut child);
                return Err(err);
            }
        };
        let stderr_reader = match spawn_stream_reader(stderr, "stderr") {
            Ok(reader) => reader,
            Err(err) => {
                terminate_after_reader_start_failure(&mut child);
                let _ = join_stream_reader(stdout_reader, "stdout");
                return Err(err);
            }
        };

        let mut timed_out = false;
        let status = loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|err| FaultlineError::Probe(err.to_string()))?
            {
                break status;
            }
            if started.elapsed() >= timeout {
                timed_out = true;
                let _ = child.kill();
                break child
                    .wait()
                    .map_err(|err| FaultlineError::Probe(err.to_string()))?;
            }
            thread::sleep(Duration::from_millis(50));
        };

        let stdout_bytes = join_stream_reader(stdout_reader, "stdout")?;
        let stderr_bytes = join_stream_reader(stderr_reader, "stderr")?;
        let exit_code = status.code();

        #[cfg(unix)]
        let signal_number = {
            use std::os::unix::process::ExitStatusExt;
            status.signal()
        };
        #[cfg(not(unix))]
        let signal_number: Option<i32> = None;

        let class = classify(exit_code, timed_out, signal_number);

        let raw_stdout = String::from_utf8_lossy(&stdout_bytes).into_owned();
        let raw_stderr = String::from_utf8_lossy(&stderr_bytes).into_owned();

        Ok(ProbeObservation {
            commit: checkout.commit.clone(),
            class,
            kind,
            exit_code,
            timed_out,
            duration_ms: started.elapsed().as_millis() as u64,
            stdout: truncate_output(&raw_stdout, truncation_limit),
            stderr: truncate_output(&raw_stderr, truncation_limit),
            sequence_index: 0,
            signal_number,
            probe_command: format_probe_command(probe),
            working_dir: checkout.path.display().to_string(),
            flake_signal: None,
        })
    }
}

impl ProbePort for ExecProbeAdapter {
    fn run(&self, checkout: &CheckedOutRevision, probe: &ProbeSpec) -> Result<ProbeObservation> {
        self.run_with_limit(checkout, probe, DEFAULT_TRUNCATION_LIMIT)
    }
}

/// Returns the effective command string for diagnostic reproducibility.
///
/// For `Exec` probes, returns the program followed by its arguments (e.g., `"cargo test --lib"`).
/// For `Shell` probes, returns the shell invocation with the script (e.g., `"sh -c 'cargo test'"`).
pub fn format_probe_command(probe: &ProbeSpec) -> String {
    match probe {
        ProbeSpec::Exec { program, args, .. } => {
            if args.is_empty() {
                program.clone()
            } else {
                format!("{} {}", program, args.join(" "))
            }
        }
        ProbeSpec::Shell { shell, script, .. } => {
            let (program, flag): (&str, &str) = match shell {
                ShellKind::Default => {
                    if cfg!(windows) {
                        ("cmd", "/C")
                    } else {
                        ("sh", "-c")
                    }
                }
                ShellKind::PosixSh => ("sh", "-c"),
                ShellKind::Cmd => ("cmd", "/C"),
                ShellKind::PowerShell => ("powershell", "-Command"),
            };
            format!("{} {} '{}'", program, flag, script)
        }
    }
}

fn build_command(probe: &ProbeSpec) -> Command {
    match probe {
        ProbeSpec::Exec {
            program, args, env, ..
        } => {
            let mut cmd = Command::new(program);
            cmd.args(args);
            for (key, value) in env {
                cmd.env(key, value);
            }
            cmd
        }
        ProbeSpec::Shell {
            shell, script, env, ..
        } => {
            let (program, args): (&str, Vec<&str>) = match shell {
                ShellKind::Default => {
                    if cfg!(windows) {
                        ("cmd", vec!["/C", script.as_str()])
                    } else {
                        ("sh", vec!["-c", script.as_str()])
                    }
                }
                ShellKind::PosixSh => ("sh", vec!["-c", script.as_str()]),
                ShellKind::Cmd => ("cmd", vec!["/C", script.as_str()]),
                ShellKind::PowerShell => ("powershell", vec!["-Command", script.as_str()]),
            };
            let mut cmd = Command::new(program);
            cmd.args(args);
            for (key, value) in env {
                cmd.env(key, value);
            }
            cmd
        }
    }
}

pub fn classify(
    exit_code: Option<i32>,
    timed_out: bool,
    signal_number: Option<i32>,
) -> ObservationClass {
    if timed_out {
        ObservationClass::Indeterminate
    } else {
        match exit_code {
            Some(0) => ObservationClass::Pass,
            Some(125) => ObservationClass::Skip,
            Some(_) => ObservationClass::Fail,
            None if signal_number.is_some() => {
                // Signal kill without timeout — Indeterminate
                ObservationClass::Indeterminate
            }
            None => {
                // Unknown termination — Indeterminate
                ObservationClass::Indeterminate
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use faultline_codes::{ObservationClass, ProbeKind};
    use faultline_types::{CheckedOutRevision, CommitId, ShellKind};
    use proptest::prelude::*;

    // Feature: v01-release-train, Property 1: Exit Code Classification
    // **Validates: Requirements 2.3, 2.4, 2.5, 2.6**
    fn exit_code_strategy() -> impl Strategy<Value = Option<i32>> {
        prop_oneof![Just(None), any::<i32>().prop_map(Some),]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_exit_code_classification(exit_code in exit_code_strategy(), timed_out in any::<bool>()) {
            let result = classify(exit_code, timed_out, None);

            let expected = if timed_out {
                ObservationClass::Indeterminate
            } else {
                match exit_code {
                    Some(0) => ObservationClass::Pass,
                    Some(125) => ObservationClass::Skip,
                    Some(_) => ObservationClass::Fail,
                    None => ObservationClass::Indeterminate,
                }
            };

            prop_assert_eq!(result, expected,
                "classify({:?}, {}) returned {:?}, expected {:?}",
                exit_code, timed_out, result, expected
            );
        }
    }

    // Feature: v01-hardening, Property 27: Signal-Aware Exit Code Classification
    // **Validates: Requirements 5.1, 5.2**
    fn signal_number_strategy() -> impl Strategy<Value = Option<i32>> {
        prop_oneof![Just(None), any::<i32>().prop_map(Some),]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_signal_aware_exit_code_classification(
            exit_code in exit_code_strategy(),
            timed_out in any::<bool>(),
            signal_number in signal_number_strategy(),
        ) {
            let result = classify(exit_code, timed_out, signal_number);

            let expected = if timed_out {
                ObservationClass::Indeterminate
            } else {
                match exit_code {
                    Some(0) => ObservationClass::Pass,
                    Some(125) => ObservationClass::Skip,
                    Some(_) => ObservationClass::Fail,
                    None if signal_number.is_some() => ObservationClass::Indeterminate,
                    None => ObservationClass::Indeterminate,
                }
            };

            prop_assert_eq!(result, expected,
                "classify({:?}, {}, {:?}) returned {:?}, expected {:?}",
                exit_code, timed_out, signal_number, result, expected
            );
        }
    }

    // Feature: v01-release-train, Property 2: Observation Structural Completeness
    // Feature: v01-hardening, Property 28: Observation Structural Completeness (Extended)
    // **Validates: Requirements 2.7, 4.7, 5.4**

    fn arb_commit_id() -> impl Strategy<Value = CommitId> {
        "[a-f0-9]{8,40}".prop_map(CommitId)
    }

    fn arb_probe_kind() -> impl Strategy<Value = ProbeKind> {
        prop_oneof![
            Just(ProbeKind::Build),
            Just(ProbeKind::Test),
            Just(ProbeKind::Lint),
            Just(ProbeKind::PerfThreshold),
            Just(ProbeKind::Custom),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_observation_structural_completeness(
            commit_id in arb_commit_id(),
            kind in arb_probe_kind(),
        ) {
            let tmp_dir = tempfile::tempdir().expect("failed to create temp dir");

            let probe = ProbeSpec::Shell {
                kind,
                shell: ShellKind::Default,
                script: "echo hello".to_string(),
                env: vec![],
                timeout_seconds: 30,
            };

            let checkout = CheckedOutRevision {
                commit: commit_id.clone(),
                path: tmp_dir.path().to_path_buf(),
            };

            let adapter = ExecProbeAdapter;
            let obs = adapter.run(&checkout, &probe).expect("probe should succeed");

            // commit is non-empty
            prop_assert!(!obs.commit.0.is_empty(), "commit must be non-empty");
            // commit matches the input
            prop_assert_eq!(&obs.commit, &commit_id, "commit must match input");

            // exit_code is Some (process exited normally, no timeout)
            prop_assert!(obs.exit_code.is_some(), "exit_code must be Some for a normally exiting process");

            // timed_out is false (echo hello completes well within 30s)
            prop_assert!(!obs.timed_out, "timed_out must be false for a fast command");

            // duration_ms is non-negative (always true for u64, but verify it was set)
            // duration_ms >= 0 is trivially true for u64

            // stdout and stderr are captured strings
            // stdout should contain "hello" from echo
            prop_assert!(obs.stdout.contains("hello"), "stdout must capture echo output, got: {:?}", obs.stdout);
            // stderr is a String (may be empty, that's fine)

            // class matches the exit code classification
            let expected_class = classify(obs.exit_code, obs.timed_out, obs.signal_number);
            prop_assert_eq!(obs.class, expected_class,
                "class must match classify(exit_code={:?}, timed_out={}, signal_number={:?})",
                obs.exit_code, obs.timed_out, obs.signal_number
            );

            // kind matches the input probe kind
            prop_assert_eq!(obs.kind, kind, "kind must match the probe spec kind");

            // Property 28 (Extended): probe_command is non-empty
            prop_assert!(!obs.probe_command.is_empty(), "probe_command must be non-empty for diagnostic reproducibility");

            // Property 28 (Extended): working_dir is non-empty
            prop_assert!(!obs.working_dir.is_empty(), "working_dir must be non-empty for diagnostic reproducibility");

            // Property 28 (Extended): working_dir matches the checkout path
            prop_assert_eq!(obs.working_dir, checkout.path.display().to_string(),
                "working_dir must match the checkout path");
        }
    }

    #[test]
    fn truncate_output_under_limit_unchanged() {
        let input = "short output";
        let result = truncate_output(input, DEFAULT_TRUNCATION_LIMIT);
        assert_eq!(result, input);
    }

    #[test]
    fn truncate_output_at_limit_unchanged() {
        let input = "a".repeat(DEFAULT_TRUNCATION_LIMIT);
        let result = truncate_output(&input, DEFAULT_TRUNCATION_LIMIT);
        assert_eq!(result, input);
    }

    #[test]
    fn truncate_output_over_limit_truncated() {
        let input = "a".repeat(DEFAULT_TRUNCATION_LIMIT + 100);
        let result = truncate_output(&input, DEFAULT_TRUNCATION_LIMIT);
        assert_eq!(
            result.len(),
            DEFAULT_TRUNCATION_LIMIT + TRUNCATION_MARKER.len()
        );
        assert!(result.ends_with(TRUNCATION_MARKER));
        assert_eq!(
            &result[..DEFAULT_TRUNCATION_LIMIT],
            &"a".repeat(DEFAULT_TRUNCATION_LIMIT)
        );

        // The byte limit falls inside this four-byte crab emoji. Truncation must
        // back up to the preceding character boundary instead of panicking.
        let utf8_input = format!("{}🦀tail", "a".repeat(62));
        let utf8_result = truncate_output(&utf8_input, 64);
        assert_eq!(
            utf8_result,
            format!("{}{}", "a".repeat(62), TRUNCATION_MARKER)
        );
    }

    #[test]
    fn truncate_output_empty_string() {
        let result = truncate_output("", DEFAULT_TRUNCATION_LIMIT);
        assert_eq!(result, "");
    }

    // Task 9.6: Probe executor hardening unit tests

    #[cfg(unix)]
    #[test]
    fn signal_termination_sets_signal_number() {
        use faultline_codes::ProbeKind;

        let tmp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let checkout = CheckedOutRevision {
            commit: CommitId("abc123".to_string()),
            path: tmp_dir.path().to_path_buf(),
        };

        // Spawn a shell probe that sleeps long enough for us to kill it.
        // The probe adapter polls every 50ms and has a 30s timeout, so
        // we use a short sleep (60s) and rely on the timeout being 1s to
        // trigger a kill. Instead, we use a trick: spawn a process that
        // sends itself SIGTERM immediately.
        let probe = ProbeSpec::Shell {
            kind: ProbeKind::Custom,
            shell: ShellKind::PosixSh,
            script: "kill -TERM $$".to_string(),
            env: vec![],
            timeout_seconds: 10,
        };

        let adapter = ExecProbeAdapter;
        let obs = adapter
            .run(&checkout, &probe)
            .expect("probe should complete");

        // The process killed itself with SIGTERM (signal 15).
        assert!(
            obs.signal_number.is_some(),
            "signal_number should be set when process is killed by a signal, got: {:?}",
            obs.signal_number
        );
        assert_eq!(
            obs.signal_number,
            Some(15),
            "signal_number should be SIGTERM (15)"
        );
        assert_eq!(
            obs.class,
            ObservationClass::Indeterminate,
            "signal-killed process should be classified as Indeterminate"
        );
        assert!(!obs.timed_out, "should not be marked as timed out");
    }

    #[test]
    fn probe_output_exceeding_limit_is_truncated() {
        // Generate substantially more data than an operating-system pipe can
        // ordinarily buffer on both stdout and stderr. The adapter must drain
        // both streams while the child runs rather than false-timeout waiting
        // for a producer blocked on a full pipe.
        use faultline_codes::ProbeKind;

        let tmp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let checkout = CheckedOutRevision {
            commit: CommitId("def456".to_string()),
            path: tmp_dir.path().to_path_buf(),
        };

        let stdout_chunk = "A".repeat(1024);
        let stderr_chunk = "B".repeat(1024);
        let (shell, script) = if cfg!(windows) {
            (
                ShellKind::PowerShell,
                format!(
                    "$outChunk='{stdout_chunk}'; $errChunk='{stderr_chunk}'; \
                     1..2048 | ForEach-Object {{ \
                     [Console]::Out.Write($outChunk); \
                     [Console]::Error.Write($errChunk) }}"
                ),
            )
        } else {
            (
                ShellKind::PosixSh,
                format!(
                    "i=0; while [ \"$i\" -lt 2048 ]; do \
                     printf '%s' '{stdout_chunk}'; \
                     printf '%s' '{stderr_chunk}' >&2; \
                     i=$((i + 1)); done"
                ),
            )
        };

        let probe = ProbeSpec::Shell {
            kind: ProbeKind::Custom,
            shell,
            script,
            env: vec![],
            timeout_seconds: 30,
        };

        let small_limit: usize = 256;
        let adapter = ExecProbeAdapter;
        let obs = adapter
            .run_with_limit(&checkout, &probe, small_limit)
            .expect("probe should complete");

        assert!(
            !obs.timed_out,
            "large output should not cause a false timeout"
        );
        assert_eq!(obs.class, ObservationClass::Pass);
        assert!(
            obs.stdout.ends_with(TRUNCATION_MARKER),
            "stdout should end with the truncation marker"
        );
        assert!(
            obs.stderr.ends_with(TRUNCATION_MARKER),
            "stderr should end with the truncation marker"
        );
        assert_eq!(
            obs.stdout.len(),
            small_limit + TRUNCATION_MARKER.len(),
            "stdout preview should be exactly limit + marker length"
        );
        assert_eq!(
            obs.stderr.len(),
            small_limit + TRUNCATION_MARKER.len(),
            "stderr preview should be exactly limit + marker length"
        );
    }
}
