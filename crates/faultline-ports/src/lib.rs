use faultline_types::{
    AnalysisReport, AnalysisRequest, CheckedOutRevision, CommitId, PathChange, ProbeObservation,
    ProbeSpec, Result, RevisionSequence, RevisionSpec, RunHandle, HistoryMode,
};

pub trait HistoryPort {
    fn linearize(
        &self,
        good: &RevisionSpec,
        bad: &RevisionSpec,
        mode: HistoryMode,
    ) -> Result<RevisionSequence>;

    fn changed_paths(&self, from: &CommitId, to: &CommitId) -> Result<Vec<PathChange>>;
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
}
