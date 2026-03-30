use faultline_codes::{AmbiguityReason, ObservationClass};
use faultline_types::{
    CommitId, Confidence, FaultlineError, LocalizationOutcome, ProbeObservation, Result,
    RevisionSequence, SearchPolicy,
};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone)]
pub struct LocalizationSession {
    sequence: RevisionSequence,
    policy: SearchPolicy,
    observations: BTreeMap<usize, ProbeObservation>,
    index_by_commit: HashMap<CommitId, usize>,
}

impl LocalizationSession {
    pub fn new(sequence: RevisionSequence, policy: SearchPolicy) -> Result<Self> {
        if sequence.revisions.is_empty() {
            return Err(FaultlineError::Domain(
                "revision sequence must not be empty".to_string(),
            ));
        }
        let index_by_commit = sequence
            .revisions
            .iter()
            .cloned()
            .enumerate()
            .map(|(idx, commit)| (commit, idx))
            .collect();
        Ok(Self {
            sequence,
            policy,
            observations: BTreeMap::new(),
            index_by_commit,
        })
    }

    pub fn has_observation(&self, commit: &CommitId) -> bool {
        self.index_of(commit)
            .map(|idx| self.observations.contains_key(&idx))
            .unwrap_or(false)
    }

    pub fn get_observation(&self, commit: &CommitId) -> Option<&ProbeObservation> {
        self.index_of(commit)
            .and_then(|idx| self.observations.get(&idx))
    }

    pub fn record(&mut self, observation: ProbeObservation) -> Result<()> {
        let idx = self.index_of(&observation.commit).ok_or_else(|| {
            FaultlineError::Domain(format!(
                "observation commit {} is not part of the revision sequence",
                observation.commit.0
            ))
        })?;
        self.observations.insert(idx, observation);
        Ok(())
    }

    pub fn observation_list(&self) -> Vec<ProbeObservation> {
        self.observations.values().cloned().collect()
    }

    pub fn sequence(&self) -> &RevisionSequence {
        &self.sequence
    }

    pub fn next_probe(&self) -> Option<CommitId> {
        if self.sequence.len() == 1 {
            return None;
        }

        let first = self.sequence.revisions.first()?.clone();
        if !self.has_observation(&first) {
            return Some(first);
        }

        let last = self.sequence.revisions.last()?.clone();
        if !self.has_observation(&last) {
            return Some(last);
        }

        let (lower, upper, _) = self.boundaries_and_reasons();
        if let (Some(lower), Some(upper)) = (lower, upper) {
            if upper <= lower + 1 {
                return None;
            }
            let candidates: Vec<usize> = ((lower + 1)..upper)
                .filter(|idx| !self.observations.contains_key(idx))
                .collect();
            if candidates.is_empty() {
                return None;
            }
            let midpoint = candidates[candidates.len() / 2];
            return self.sequence.revisions.get(midpoint).cloned();
        }

        let candidates: Vec<usize> = (0..self.sequence.len())
            .filter(|idx| !self.observations.contains_key(idx))
            .collect();
        if candidates.is_empty() {
            None
        } else {
            let midpoint = candidates[candidates.len() / 2];
            self.sequence.revisions.get(midpoint).cloned()
        }
    }

    pub fn outcome(&self) -> LocalizationOutcome {
        let (lower, upper, mut reasons) = self.boundaries_and_reasons();

        let Some(lower) = lower else {
            if reasons.is_empty() {
                reasons.push(AmbiguityReason::MissingPassBoundary);
            }
            return LocalizationOutcome::Inconclusive { reasons };
        };

        let Some(upper) = upper else {
            if reasons.is_empty() {
                reasons.push(AmbiguityReason::MissingFailBoundary);
            }
            return LocalizationOutcome::Inconclusive { reasons };
        };

        let mut skipped_between = false;
        let mut indeterminate_between = false;
        let mut unknown_between = false;

        for idx in (lower + 1)..upper {
            match self.observations.get(&idx).map(|obs| obs.class) {
                None => unknown_between = true,
                Some(ObservationClass::Skip) => skipped_between = true,
                Some(ObservationClass::Indeterminate) => indeterminate_between = true,
                _ => {}
            }
        }

        if unknown_between {
            reasons.push(AmbiguityReason::NeedsMoreProbes);
            return LocalizationOutcome::Inconclusive { reasons };
        }

        if skipped_between {
            reasons.push(AmbiguityReason::SkippedRevision);
        }
        if indeterminate_between {
            reasons.push(AmbiguityReason::IndeterminateRevision);
        }

        let lower_commit = self.sequence.revisions[lower].clone();
        let upper_commit = self.sequence.revisions[upper].clone();

        if upper == lower + 1 && reasons.is_empty() {
            return LocalizationOutcome::FirstBad {
                last_good: lower_commit,
                first_bad: upper_commit,
                confidence: Confidence::high(),
            };
        }

        if reasons.is_empty() {
            LocalizationOutcome::FirstBad {
                last_good: lower_commit,
                first_bad: upper_commit,
                confidence: Confidence::high(),
            }
        } else {
            LocalizationOutcome::SuspectWindow {
                lower_bound_exclusive: lower_commit,
                upper_bound_inclusive: upper_commit,
                confidence: if reasons.iter().any(|reason| {
                    matches!(reason, AmbiguityReason::NonMonotonicEvidence)
                }) {
                    Confidence::low()
                } else {
                    Confidence::medium()
                },
                reasons,
            }
        }
    }

    fn index_of(&self, commit: &CommitId) -> Option<usize> {
        self.index_by_commit.get(commit).copied()
    }

    fn boundaries_and_reasons(&self) -> (Option<usize>, Option<usize>, Vec<AmbiguityReason>) {
        let mut reasons = Vec::new();
        let mut pass_indices = Vec::new();
        let mut fail_indices = Vec::new();

        for (idx, obs) in &self.observations {
            match obs.class {
                ObservationClass::Pass => pass_indices.push(*idx),
                ObservationClass::Fail => fail_indices.push(*idx),
                ObservationClass::Skip => reasons.push(AmbiguityReason::SkippedRevision),
                ObservationClass::Indeterminate => reasons.push(AmbiguityReason::IndeterminateRevision),
            }
        }

        pass_indices.sort_unstable();
        fail_indices.sort_unstable();

        let lower = pass_indices.last().copied();
        let upper = lower.and_then(|current_lower| {
            fail_indices
                .iter()
                .copied()
                .find(|candidate| *candidate > current_lower)
        });

        if lower.is_none() {
            reasons.push(AmbiguityReason::MissingPassBoundary);
        }
        if upper.is_none() {
            reasons.push(AmbiguityReason::MissingFailBoundary);
        }

        if let (Some(lower), Some(min_fail)) = (lower, fail_indices.first().copied()) {
            if min_fail < lower {
                reasons.push(AmbiguityReason::NonMonotonicEvidence);
            }
        }

        reasons.sort_by_key(|reason| reason.to_string());
        reasons.dedup();

        (lower, upper, reasons)
    }

    pub fn max_probes(&self) -> usize {
        self.policy.max_probes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use faultline_codes::{ObservationClass, ProbeKind};
    use faultline_types::{ProbeObservation, SearchPolicy};

    fn obs(commit: &str, class: ObservationClass) -> ProbeObservation {
        ProbeObservation {
            commit: CommitId(commit.to_string()),
            class,
            kind: ProbeKind::Test,
            exit_code: Some(match class {
                ObservationClass::Pass => 0,
                ObservationClass::Skip => 125,
                _ => 1,
            }),
            timed_out: matches!(class, ObservationClass::Indeterminate),
            duration_ms: 1,
            stdout: String::new(),
            stderr: String::new(),
        }
    }

    #[test]
    fn exact_boundary_when_adjacent() {
        let seq = RevisionSequence {
            revisions: vec![
                CommitId("a".into()),
                CommitId("b".into()),
                CommitId("c".into()),
            ],
        };
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        session.record(obs("c", ObservationClass::Fail)).unwrap();
        assert_eq!(session.next_probe(), Some(CommitId("b".into())));
        session.record(obs("b", ObservationClass::Fail)).unwrap();
        match session.outcome() {
            LocalizationOutcome::FirstBad { first_bad, .. } => assert_eq!(first_bad.0, "b"),
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn skipped_midpoint_yields_suspect_window() {
        let seq = RevisionSequence {
            revisions: vec![
                CommitId("a".into()),
                CommitId("b".into()),
                CommitId("c".into()),
            ],
        };
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        session.record(obs("b", ObservationClass::Skip)).unwrap();
        session.record(obs("c", ObservationClass::Fail)).unwrap();
        match session.outcome() {
            LocalizationOutcome::SuspectWindow { .. } => {}
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
}
