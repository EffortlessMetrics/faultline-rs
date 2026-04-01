use std::collections::HashMap;

use faultline_codes::ObservationClass;
use faultline_types::{
    AnalysisReport, AnalysisRequest, CheckedOutRevision, CommitId, HistoryMode, PathChange,
    ProbeObservation, ProbeSpec, Result, RevisionSequence, RevisionSpec, RunHandle,
};

pub trait HistoryPort {
    fn linearize(
        &self,
        good: &RevisionSpec,
        bad: &RevisionSpec,
        mode: HistoryMode,
    ) -> Result<RevisionSequence>;

    fn changed_paths(&self, from: &CommitId, to: &CommitId) -> Result<Vec<PathChange>>;

    /// Parse CODEOWNERS and return owner for each path. Returns empty map if no CODEOWNERS.
    fn codeowners_for_paths(&self, paths: &[String]) -> Result<HashMap<String, Option<String>>>;

    /// Derive owner from git-blame frequency (most-frequent committer in last 90 days).
    fn blame_frequency(&self, paths: &[String]) -> Result<HashMap<String, Option<String>>>;
}

pub trait CheckoutPort {
    fn checkout_revision(&self, commit: &CommitId) -> Result<CheckedOutRevision>;
    fn cleanup_checkout(&self, checkout: &CheckedOutRevision) -> Result<()>;
}

pub trait ProbePort {
    fn run(&self, checkout: &CheckedOutRevision, probe: &ProbeSpec) -> Result<ProbeObservation>;
}

pub trait RunStorePort {
    fn prepare_run(&self, request: &AnalysisRequest) -> Result<RunHandle>;
    fn load_observations(&self, run: &RunHandle) -> Result<Vec<ProbeObservation>>;
    fn save_observation(&self, run: &RunHandle, observation: &ProbeObservation) -> Result<()>;
    fn save_report(&self, run: &RunHandle, report: &AnalysisReport) -> Result<()>;
    fn load_report(&self, run: &RunHandle) -> Result<Option<AnalysisReport>>;
    /// Persist full probe stdout/stderr to per-commit log files.
    /// Called by the app layer when truncated output is detected.
    fn save_probe_logs(
        &self,
        run: &RunHandle,
        commit_sha: &str,
        stdout: &str,
        stderr: &str,
    ) -> Result<()>;
    /// Clear all cached observations for a run (used by --force).
    fn clear_observations(&self, run: &RunHandle) -> Result<()>;
    /// Delete the entire run directory (used by --fresh).
    fn delete_run(&self, run: &RunHandle) -> Result<()>;
}

/// Callback port for reporting localization progress to the user.
pub trait ProgressPort {
    fn on_probe_start(&self, commit: &CommitId, probe_index: usize, total_estimate: usize);
    fn on_probe_complete(&self, commit: &CommitId, class: ObservationClass, duration_ms: u64);
    fn on_session_complete(&self, total_probes: usize);
}

/// No-op implementation that silently discards all progress events.
pub struct SilentProgress;

impl ProgressPort for SilentProgress {
    fn on_probe_start(&self, _: &CommitId, _: usize, _: usize) {}
    fn on_probe_complete(&self, _: &CommitId, _: ObservationClass, _: u64) {}
    fn on_session_complete(&self, _: usize) {}
}
