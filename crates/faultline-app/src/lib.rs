use faultline_codes::ObservationClass;
use faultline_localization::LocalizationSession;
use faultline_ports::{CheckoutPort, HistoryPort, ProbePort, RunStorePort};
use faultline_surface::SurfaceAnalyzer;
use faultline_types::{
    now_epoch_seconds, AnalysisReport, AnalysisRequest, FaultlineError, LocalizationOutcome,
    Result, RunHandle,
};

#[derive(Debug, Clone)]
pub struct LocalizedRun {
    pub run: RunHandle,
    pub report: AnalysisReport,
}

pub struct FaultlineApp<'a> {
    history: &'a dyn HistoryPort,
    checkout: &'a dyn CheckoutPort,
    probe: &'a dyn ProbePort,
    store: &'a dyn RunStorePort,
    surface: SurfaceAnalyzer,
}

impl<'a> FaultlineApp<'a> {
    pub fn new(
        history: &'a dyn HistoryPort,
        checkout: &'a dyn CheckoutPort,
        probe: &'a dyn ProbePort,
        store: &'a dyn RunStorePort,
    ) -> Self {
        Self {
            history,
            checkout,
            probe,
            store,
            surface: SurfaceAnalyzer,
        }
    }

    pub fn localize(&self, request: AnalysisRequest) -> Result<LocalizedRun> {
        let run = self.store.prepare_run(&request)?;
        let sequence = self
            .history
            .linearize(&request.good, &request.bad, request.history_mode)?;

        let mut session = LocalizationSession::new(sequence.clone(), request.policy.clone())?;
        for observation in self.store.load_observations(&run)? {
            session.record(observation)?;
        }

        self.ensure_boundary(
            &run,
            &request,
            &mut session,
            0,
            ObservationClass::Pass,
            "known-good",
        )?;
        self.ensure_boundary(
            &run,
            &request,
            &mut session,
            sequence.len() - 1,
            ObservationClass::Fail,
            "known-bad",
        )?;

        let mut probe_count = session.observation_list().len();
        while probe_count < session.max_probes() {
            let Some(commit) = session.next_probe() else {
                break;
            };
            if session.has_observation(&commit) {
                break;
            }
            let observation = self.probe_commit(&request, &commit)?;
            self.store.save_observation(&run, &observation)?;
            session.record(observation)?;
            probe_count += 1;
            match session.outcome() {
                LocalizationOutcome::FirstBad { .. } | LocalizationOutcome::SuspectWindow { .. } => {
                    if session.next_probe().is_none() {
                        break;
                    }
                }
                LocalizationOutcome::Inconclusive { .. } => {}
            }
        }

        let outcome = session.outcome();
        let changed_paths = if let Some((from, to)) = outcome.boundary_pair() {
            self.history.changed_paths(from, to)?
        } else {
            Vec::new()
        };
        let surface = self.surface.summarize(&changed_paths);
        let report = AnalysisReport {
            run_id: run.id.clone(),
            created_at_epoch_seconds: now_epoch_seconds(),
            request,
            sequence,
            observations: session.observation_list(),
            outcome,
            changed_paths,
            surface,
        };
        self.store.save_report(&run, &report)?;
        Ok(LocalizedRun { run, report })
    }

    fn ensure_boundary(
        &self,
        run: &RunHandle,
        request: &AnalysisRequest,
        session: &mut LocalizationSession,
        index: usize,
        expected: ObservationClass,
        label: &str,
    ) -> Result<()> {
        let commit = session
            .sequence()
            .revisions
            .get(index)
            .ok_or_else(|| FaultlineError::Domain("missing boundary index".to_string()))?
            .clone();

        if !session.has_observation(&commit) {
            let observation = self.probe_commit(request, &commit)?;
            self.store.save_observation(run, &observation)?;
            session.record(observation)?;
        }

        let observed = session
            .get_observation(&commit)
            .ok_or_else(|| FaultlineError::Domain("boundary observation missing".to_string()))?;
        if observed.class != expected {
            return Err(FaultlineError::InvalidBoundary(format!(
                "{label} boundary {} evaluated as {:?}; expected {:?}",
                commit.0, observed.class, expected
            )));
        }
        Ok(())
    }

    fn probe_commit(
        &self,
        request: &AnalysisRequest,
        commit: &faultline_types::CommitId,
    ) -> Result<faultline_types::ProbeObservation> {
        let checkout = self.checkout.checkout_revision(commit)?;
        let result = self.probe.run(&checkout, &request.probe);
        let cleanup = self.checkout.cleanup_checkout(&checkout);
        match (result, cleanup) {
            (Ok(observation), Ok(())) => Ok(observation),
            (Err(err), Ok(())) => Err(err),
            (Ok(_), Err(cleanup_err)) => Err(cleanup_err),
            (Err(err), Err(_cleanup_err)) => Err(err),
        }
    }
}
