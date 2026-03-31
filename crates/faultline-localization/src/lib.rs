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
        let mut list: Vec<ProbeObservation> = self.observations.values().cloned().collect();
        list.sort_by_key(|o| o.sequence_index);
        list
    }

    pub fn sequence(&self) -> &RevisionSequence {
        &self.sequence
    }

    pub fn max_probes(&self) -> usize {
        self.policy.max_probes
    }

    /// Select the next commit to probe using binary narrowing.
    ///
    /// 1. Return `None` if max_probes reached.
    /// 2. Ensure boundaries are probed first (first and last in sequence).
    /// 3. Find the tightest pass/fail boundary pair.
    /// 4. Collect unobserved indices between boundaries.
    /// 5. Return the median unobserved index (binary search midpoint).
    /// 6. Return `None` when no unobserved candidates remain.
    pub fn next_probe(&self) -> Option<CommitId> {
        // Stop if max probes reached
        if self.observations.len() >= self.policy.max_probes {
            return None;
        }

        if self.sequence.len() == 1 {
            return None;
        }

        // Ensure boundaries are probed first
        let first = self.sequence.revisions.first()?.clone();
        if !self.has_observation(&first) {
            return Some(first);
        }

        let last = self.sequence.revisions.last()?.clone();
        if !self.has_observation(&last) {
            return Some(last);
        }

        // Find tightest pass/fail boundary pair
        let (lower, upper) = self.boundary_indices();
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

        // Fallback: no boundary pair yet, pick median unobserved
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

    /// Determine the localization outcome based on current observations.
    pub fn outcome(&self) -> LocalizationOutcome {
        let max_probes_exhausted = self.observations.len() >= self.policy.max_probes;
        let (pass_boundary, fail_boundary, non_monotonic) = self.boundaries_and_reasons();

        // Missing boundaries → Inconclusive
        let Some(lower) = pass_boundary else {
            let mut reasons = vec![AmbiguityReason::MissingPassBoundary];
            if max_probes_exhausted {
                reasons.push(AmbiguityReason::MaxProbesExhausted);
            }
            return LocalizationOutcome::Inconclusive { reasons };
        };

        let Some(upper) = fail_boundary else {
            let mut reasons = vec![AmbiguityReason::MissingFailBoundary];
            if non_monotonic {
                reasons.push(AmbiguityReason::NonMonotonicEvidence);
            }
            if max_probes_exhausted {
                reasons.push(AmbiguityReason::MaxProbesExhausted);
            }
            return LocalizationOutcome::Inconclusive { reasons };
        };

        // Check what's between the boundaries
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

        // Unobserved commits between boundaries → Inconclusive
        if unknown_between {
            let mut reasons = vec![AmbiguityReason::NeedsMoreProbes];
            if non_monotonic {
                reasons.push(AmbiguityReason::NonMonotonicEvidence);
            }
            if max_probes_exhausted {
                reasons.push(AmbiguityReason::MaxProbesExhausted);
            }
            return LocalizationOutcome::Inconclusive { reasons };
        }

        let lower_commit = self.sequence.revisions[lower].clone();
        let upper_commit = self.sequence.revisions[upper].clone();

        // Collect reasons for SuspectWindow
        let mut reasons = Vec::new();
        if skipped_between {
            reasons.push(AmbiguityReason::SkippedRevision);
        }
        if indeterminate_between {
            reasons.push(AmbiguityReason::IndeterminateRevision);
        }
        if non_monotonic {
            reasons.push(AmbiguityReason::NonMonotonicEvidence);
        }

        if reasons.is_empty() {
            // Clean boundary: FirstBad
            LocalizationOutcome::FirstBad {
                last_good: lower_commit,
                first_bad: upper_commit,
                confidence: Confidence::high(),
            }
        } else {
            // Window has not converged to a clean adjacent pass-fail pair
            if max_probes_exhausted {
                reasons.push(AmbiguityReason::MaxProbesExhausted);
            }
            // Ambiguous: SuspectWindow
            let confidence = if non_monotonic {
                Confidence::low()
            } else {
                Confidence::medium()
            };
            LocalizationOutcome::SuspectWindow {
                lower_bound_exclusive: lower_commit,
                upper_bound_inclusive: upper_commit,
                confidence,
                reasons,
            }
        }
    }

    fn index_of(&self, commit: &CommitId) -> Option<usize> {
        self.index_by_commit.get(commit).copied()
    }

    /// Compute the pass/fail boundary indices only (no reason collection).
    fn boundary_indices(&self) -> (Option<usize>, Option<usize>) {
        let mut highest_pass: Option<usize> = None;
        let mut fail_indices = Vec::new();

        for (idx, obs) in &self.observations {
            match obs.class {
                ObservationClass::Pass => {
                    highest_pass = Some(highest_pass.map_or(*idx, |prev: usize| prev.max(*idx)));
                }
                ObservationClass::Fail => fail_indices.push(*idx),
                _ => {}
            }
        }

        let upper =
            highest_pass.and_then(|hp| fail_indices.iter().copied().filter(|&f| f > hp).min());

        (highest_pass, upper)
    }

    /// Find the highest pass index, lowest fail index > pass, and detect non-monotonic evidence.
    fn boundaries_and_reasons(&self) -> (Option<usize>, Option<usize>, bool) {
        let mut pass_indices = Vec::new();
        let mut fail_indices = Vec::new();

        for (idx, obs) in &self.observations {
            match obs.class {
                ObservationClass::Pass => pass_indices.push(*idx),
                ObservationClass::Fail => fail_indices.push(*idx),
                _ => {}
            }
        }

        let highest_pass = pass_indices.iter().copied().max();
        let lowest_fail_above_pass =
            highest_pass.and_then(|hp| fail_indices.iter().copied().filter(|&f| f > hp).min());

        // Non-monotonic: any Fail at index < any Pass index
        let non_monotonic = if let (Some(min_fail), Some(max_pass)) =
            (fail_indices.iter().copied().min(), highest_pass)
        {
            min_fail < max_pass
        } else {
            false
        };

        (highest_pass, lowest_fail_above_pass, non_monotonic)
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
            sequence_index: 0,
            signal_number: None,
            probe_command: String::new(),
            working_dir: String::new(),
        }
    }

    fn obs_with_seq(commit: &str, class: ObservationClass, seq_idx: u64) -> ProbeObservation {
        let mut o = obs(commit, class);
        o.sequence_index = seq_idx;
        o
    }

    fn make_seq(labels: &[&str]) -> RevisionSequence {
        RevisionSequence {
            revisions: labels.iter().map(|l| CommitId(l.to_string())).collect(),
        }
    }

    // --- new ---

    #[test]
    fn new_rejects_empty_sequence() {
        let seq = RevisionSequence { revisions: vec![] };
        assert!(LocalizationSession::new(seq, SearchPolicy::default()).is_err());
    }

    #[test]
    fn new_builds_index_by_commit() {
        let seq = make_seq(&["a", "b", "c"]);
        let session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        assert_eq!(session.index_of(&CommitId("a".into())), Some(0));
        assert_eq!(session.index_of(&CommitId("b".into())), Some(1));
        assert_eq!(session.index_of(&CommitId("c".into())), Some(2));
        assert_eq!(session.index_of(&CommitId("z".into())), None);
    }

    // --- record ---

    #[test]
    fn record_rejects_unknown_commit() {
        let seq = make_seq(&["a", "b"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        assert!(session.record(obs("z", ObservationClass::Pass)).is_err());
    }

    #[test]
    fn record_accepts_known_commit() {
        let seq = make_seq(&["a", "b"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        assert!(session.record(obs("a", ObservationClass::Pass)).is_ok());
    }

    // --- accessors ---

    #[test]
    fn has_observation_and_get_observation() {
        let seq = make_seq(&["a", "b", "c"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        assert!(!session.has_observation(&CommitId("a".into())));
        assert!(session.get_observation(&CommitId("a".into())).is_none());

        session.record(obs("a", ObservationClass::Pass)).unwrap();
        assert!(session.has_observation(&CommitId("a".into())));
        assert_eq!(
            session
                .get_observation(&CommitId("a".into()))
                .unwrap()
                .class,
            ObservationClass::Pass
        );
    }

    #[test]
    fn observation_list_returns_all_in_sequence_index_order() {
        let seq = make_seq(&["a", "b", "c"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        // Record "c" first with sequence_index=0, then "a" with sequence_index=1
        session
            .record(obs_with_seq("c", ObservationClass::Fail, 0))
            .unwrap();
        session
            .record(obs_with_seq("a", ObservationClass::Pass, 1))
            .unwrap();
        let list = session.observation_list();
        assert_eq!(list.len(), 2);
        // Ordered by sequence_index: c(0) before a(1)
        assert_eq!(list[0].commit.0, "c");
        assert_eq!(list[1].commit.0, "a");
    }

    #[test]
    fn observation_list_orders_by_sequence_index_not_revision_position() {
        let seq = make_seq(&["a", "b", "c", "d", "e"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        // Simulate probing in binary-search order: e(seq=0), a(seq=1), c(seq=2), b(seq=3), d(seq=4)
        session
            .record(obs_with_seq("e", ObservationClass::Fail, 0))
            .unwrap();
        session
            .record(obs_with_seq("a", ObservationClass::Pass, 1))
            .unwrap();
        session
            .record(obs_with_seq("c", ObservationClass::Fail, 2))
            .unwrap();
        session
            .record(obs_with_seq("b", ObservationClass::Pass, 3))
            .unwrap();
        session
            .record(obs_with_seq("d", ObservationClass::Fail, 4))
            .unwrap();
        let list = session.observation_list();
        assert_eq!(list.len(), 5);
        assert_eq!(list[0].commit.0, "e"); // seq_idx=0
        assert_eq!(list[1].commit.0, "a"); // seq_idx=1
        assert_eq!(list[2].commit.0, "c"); // seq_idx=2
        assert_eq!(list[3].commit.0, "b"); // seq_idx=3
        assert_eq!(list[4].commit.0, "d"); // seq_idx=4
    }

    #[test]
    fn record_preserves_preassigned_sequence_index() {
        let seq = make_seq(&["a", "b"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session
            .record(obs_with_seq("a", ObservationClass::Pass, 42))
            .unwrap();
        let list = session.observation_list();
        assert_eq!(list[0].sequence_index, 42);
    }

    #[test]
    fn sequence_accessor() {
        let seq = make_seq(&["a", "b"]);
        let session = LocalizationSession::new(seq.clone(), SearchPolicy::default()).unwrap();
        assert_eq!(session.sequence().revisions.len(), 2);
    }

    #[test]
    fn max_probes_accessor() {
        let seq = make_seq(&["a", "b"]);
        let policy = SearchPolicy {
            max_probes: 42,
            ..SearchPolicy::default()
        };
        let session = LocalizationSession::new(seq, policy).unwrap();
        assert_eq!(session.max_probes(), 42);
    }

    // --- next_probe ---

    #[test]
    fn next_probe_probes_first_boundary_first() {
        let seq = make_seq(&["a", "b", "c"]);
        let session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        assert_eq!(session.next_probe(), Some(CommitId("a".into())));
    }

    #[test]
    fn next_probe_probes_last_boundary_second() {
        let seq = make_seq(&["a", "b", "c"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        assert_eq!(session.next_probe(), Some(CommitId("c".into())));
    }

    #[test]
    fn next_probe_binary_narrows_between_boundaries() {
        let seq = make_seq(&["a", "b", "c", "d", "e"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        session.record(obs("e", ObservationClass::Fail)).unwrap();
        // Candidates: b(1), c(2), d(3) → median is c(2)
        assert_eq!(session.next_probe(), Some(CommitId("c".into())));
    }

    #[test]
    fn next_probe_returns_none_when_converged() {
        let seq = make_seq(&["a", "b"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        session.record(obs("b", ObservationClass::Fail)).unwrap();
        assert_eq!(session.next_probe(), None);
    }

    #[test]
    fn next_probe_respects_max_probes() {
        let seq = make_seq(&["a", "b", "c", "d", "e"]);
        let policy = SearchPolicy {
            max_probes: 2,
            ..SearchPolicy::default()
        };
        let mut session = LocalizationSession::new(seq, policy).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        session.record(obs("e", ObservationClass::Fail)).unwrap();
        // 2 observations recorded, max_probes is 2 → should return None
        assert_eq!(session.next_probe(), None);
    }

    #[test]
    fn next_probe_single_element_returns_none() {
        let seq = make_seq(&["a"]);
        let session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        assert_eq!(session.next_probe(), None);
    }

    // --- outcome ---

    #[test]
    fn exact_boundary_when_adjacent() {
        let seq = make_seq(&["a", "b", "c"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        session.record(obs("c", ObservationClass::Fail)).unwrap();
        assert_eq!(session.next_probe(), Some(CommitId("b".into())));
        session.record(obs("b", ObservationClass::Fail)).unwrap();
        match session.outcome() {
            LocalizationOutcome::FirstBad {
                first_bad,
                last_good,
                confidence,
            } => {
                assert_eq!(first_bad.0, "b");
                assert_eq!(last_good.0, "a");
                assert_eq!(confidence, Confidence::high());
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn skipped_midpoint_yields_suspect_window() {
        let seq = make_seq(&["a", "b", "c"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        session.record(obs("b", ObservationClass::Skip)).unwrap();
        session.record(obs("c", ObservationClass::Fail)).unwrap();
        match session.outcome() {
            LocalizationOutcome::SuspectWindow {
                reasons,
                confidence,
                ..
            } => {
                assert!(reasons.contains(&AmbiguityReason::SkippedRevision));
                assert_eq!(confidence, Confidence::medium());
                assert!(confidence.score < Confidence::high().score);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn indeterminate_midpoint_yields_suspect_window() {
        let seq = make_seq(&["a", "b", "c"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        session
            .record(obs("b", ObservationClass::Indeterminate))
            .unwrap();
        session.record(obs("c", ObservationClass::Fail)).unwrap();
        match session.outcome() {
            LocalizationOutcome::SuspectWindow {
                reasons,
                confidence,
                ..
            } => {
                assert!(reasons.contains(&AmbiguityReason::IndeterminateRevision));
                assert!(confidence.score < Confidence::high().score);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn missing_pass_boundary_yields_inconclusive() {
        let seq = make_seq(&["a", "b", "c"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session.record(obs("c", ObservationClass::Fail)).unwrap();
        match session.outcome() {
            LocalizationOutcome::Inconclusive { reasons } => {
                assert!(reasons.contains(&AmbiguityReason::MissingPassBoundary));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn missing_fail_boundary_yields_inconclusive() {
        let seq = make_seq(&["a", "b", "c"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        match session.outcome() {
            LocalizationOutcome::Inconclusive { reasons } => {
                assert!(reasons.contains(&AmbiguityReason::MissingFailBoundary));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn non_monotonic_evidence_yields_low_confidence() {
        // Sequence: a, b, c, d — Fail at b (idx 1), Pass at c (idx 2)
        let seq = make_seq(&["a", "b", "c", "d"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session.record(obs("b", ObservationClass::Fail)).unwrap();
        session.record(obs("c", ObservationClass::Pass)).unwrap();
        session.record(obs("d", ObservationClass::Fail)).unwrap();
        match session.outcome() {
            LocalizationOutcome::SuspectWindow {
                reasons,
                confidence,
                ..
            } => {
                assert!(reasons.contains(&AmbiguityReason::NonMonotonicEvidence));
                assert_eq!(confidence, Confidence::low());
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn unobserved_between_boundaries_yields_inconclusive() {
        let seq = make_seq(&["a", "b", "c", "d"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        session.record(obs("d", ObservationClass::Fail)).unwrap();
        // b and c are unobserved
        match session.outcome() {
            LocalizationOutcome::Inconclusive { reasons } => {
                assert!(reasons.contains(&AmbiguityReason::NeedsMoreProbes));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn first_bad_with_all_between_observed() {
        // a=Pass, b=Pass, c=Fail → FirstBad(b, c)
        let seq = make_seq(&["a", "b", "c"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        session.record(obs("b", ObservationClass::Pass)).unwrap();
        session.record(obs("c", ObservationClass::Fail)).unwrap();
        match session.outcome() {
            LocalizationOutcome::FirstBad {
                last_good,
                first_bad,
                confidence,
            } => {
                assert_eq!(last_good.0, "b");
                assert_eq!(first_bad.0, "c");
                assert_eq!(confidence, Confidence::high());
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    // --- max probes exhausted outcome ---

    #[test]
    fn max_probes_exhausted_with_unobserved_between_boundaries() {
        // 5 commits, max_probes=2, observe only endpoints → Inconclusive with MaxProbesExhausted
        let seq = make_seq(&["a", "b", "c", "d", "e"]);
        let policy = SearchPolicy {
            max_probes: 2,
            ..SearchPolicy::default()
        };
        let mut session = LocalizationSession::new(seq, policy).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        session.record(obs("e", ObservationClass::Fail)).unwrap();
        match session.outcome() {
            LocalizationOutcome::Inconclusive { reasons } => {
                assert!(reasons.contains(&AmbiguityReason::NeedsMoreProbes));
                assert!(reasons.contains(&AmbiguityReason::MaxProbesExhausted));
            }
            other => panic!("expected Inconclusive, got {other:?}"),
        }
    }

    #[test]
    fn max_probes_exhausted_with_skipped_between_boundaries() {
        // 3 commits, max_probes=3, all observed but midpoint is Skip → SuspectWindow with MaxProbesExhausted
        let seq = make_seq(&["a", "b", "c"]);
        let policy = SearchPolicy {
            max_probes: 3,
            ..SearchPolicy::default()
        };
        let mut session = LocalizationSession::new(seq, policy).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        session.record(obs("b", ObservationClass::Skip)).unwrap();
        session.record(obs("c", ObservationClass::Fail)).unwrap();
        match session.outcome() {
            LocalizationOutcome::SuspectWindow { reasons, .. } => {
                assert!(reasons.contains(&AmbiguityReason::SkippedRevision));
                assert!(reasons.contains(&AmbiguityReason::MaxProbesExhausted));
            }
            other => panic!("expected SuspectWindow, got {other:?}"),
        }
    }

    #[test]
    fn max_probes_exhausted_missing_pass_boundary() {
        // Only fail observations, max_probes reached → Inconclusive with MissingPassBoundary + MaxProbesExhausted
        let seq = make_seq(&["a", "b", "c"]);
        let policy = SearchPolicy {
            max_probes: 1,
            ..SearchPolicy::default()
        };
        let mut session = LocalizationSession::new(seq, policy).unwrap();
        session.record(obs("c", ObservationClass::Fail)).unwrap();
        match session.outcome() {
            LocalizationOutcome::Inconclusive { reasons } => {
                assert!(reasons.contains(&AmbiguityReason::MissingPassBoundary));
                assert!(reasons.contains(&AmbiguityReason::MaxProbesExhausted));
            }
            other => panic!("expected Inconclusive, got {other:?}"),
        }
    }

    #[test]
    fn max_probes_not_exhausted_no_extra_reason() {
        // Adjacent pass-fail with plenty of budget → FirstBad, no MaxProbesExhausted
        let seq = make_seq(&["a", "b"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        session.record(obs("b", ObservationClass::Fail)).unwrap();
        match session.outcome() {
            LocalizationOutcome::FirstBad { confidence, .. } => {
                assert_eq!(confidence, Confidence::high());
            }
            other => panic!("expected FirstBad, got {other:?}"),
        }
    }

    #[test]
    fn max_probes_exhausted_but_converged_yields_first_bad() {
        // Adjacent pass-fail AND max_probes reached → still FirstBad (converged)
        let seq = make_seq(&["a", "b"]);
        let policy = SearchPolicy {
            max_probes: 2,
            ..SearchPolicy::default()
        };
        let mut session = LocalizationSession::new(seq, policy).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        session.record(obs("b", ObservationClass::Fail)).unwrap();
        match session.outcome() {
            LocalizationOutcome::FirstBad {
                last_good,
                first_bad,
                confidence,
            } => {
                assert_eq!(last_good.0, "a");
                assert_eq!(first_bad.0, "b");
                assert_eq!(confidence, Confidence::high());
            }
            other => panic!("expected FirstBad, got {other:?}"),
        }
    }

    // Feature: v01-release-train, Property 4: Binary Narrowing Selects Valid Midpoint
    // **Validates: Requirements 3.1**
    mod prop_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

            // Feature: v01-release-train, Property 5: Adjacent Pass-Fail Yields FirstBad
            // **Validates: Requirements 3.2**
            #[test]
            fn prop_adjacent_pass_fail_yields_first_bad(n in 2usize..=20, i_frac in 0.0f64..1.0) {
                // Derive boundary index i in 0..n-1 so that i+1 < n
                let i = (i_frac * (n - 1) as f64).floor() as usize;
                let i = i.min(n - 2); // safety clamp

                // Build a sequence of n commits
                let labels: Vec<CommitId> = (0..n)
                    .map(|idx| CommitId(format!("commit-{idx}")))
                    .collect();
                let seq = RevisionSequence { revisions: labels.clone() };

                let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();

                // Record observations for ALL indices so everything is observed:
                // indices < i  → Pass
                // index i      → Pass
                // index i+1    → Fail
                // indices > i+1 → Fail
                for idx in 0..n {
                    let class = if idx <= i {
                        ObservationClass::Pass
                    } else {
                        ObservationClass::Fail
                    };
                    session.record(obs(&format!("commit-{idx}"), class)).unwrap();
                }

                let outcome = session.outcome();

                match outcome {
                    LocalizationOutcome::FirstBad { last_good, first_bad, confidence } => {
                        prop_assert_eq!(last_good, labels[i].clone());
                        prop_assert_eq!(first_bad, labels[i + 1].clone());
                        prop_assert_eq!(confidence, Confidence::high());
                    }
                    other => {
                        prop_assert!(false, "expected FirstBad but got {:?}", other);
                    }
                }
            }

            #[test]
            fn prop_binary_narrowing_selects_valid_midpoint(n in 3usize..=50) {
                // Build a sequence of n commits
                let labels: Vec<CommitId> = (0..n)
                    .map(|i| CommitId(format!("commit-{i}")))
                    .collect();
                let seq = RevisionSequence { revisions: labels };

                // Create session with default policy (max_probes=64, plenty of room)
                let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();

                // Record Pass at first commit, Fail at last commit
                session.record(obs("commit-0", ObservationClass::Pass)).unwrap();
                session.record(obs(&format!("commit-{}", n - 1), ObservationClass::Fail)).unwrap();

                // Call next_probe
                let probe = session.next_probe();

                // Must be Some — there are unobserved candidates between index 0 and n-1
                prop_assert!(probe.is_some(), "next_probe() should return Some for n={n}");
                let probe_commit = probe.unwrap();

                // Find the index of the returned commit
                let idx = session.index_by_commit.get(&probe_commit)
                    .copied()
                    .expect("returned commit must be in the sequence");

                // Index must be strictly between 0 and n-1
                prop_assert!(idx > 0, "probe index {idx} must be > 0");
                prop_assert!(idx < n - 1, "probe index {idx} must be < {}", n - 1);

                // Must have no existing observation
                prop_assert!(
                    !session.has_observation(&probe_commit),
                    "probe commit {:?} at index {idx} must not have an existing observation",
                    probe_commit
                );
            }

            // Feature: v01-release-train, Property 10: FirstBad Requires Direct Evidence
            // **Validates: Requirements 3.9, 11.1**
            #[test]
            fn prop_first_bad_requires_direct_evidence(n in 2usize..=20, i_frac in 0.0f64..1.0) {
                // Derive boundary index i in 0..n-1 so that i+1 < n
                let i = (i_frac * (n - 1) as f64).floor() as usize;
                let i = i.min(n - 2); // safety clamp

                // Build a sequence of n commits
                let labels: Vec<CommitId> = (0..n)
                    .map(|idx| CommitId(format!("commit-{idx}")))
                    .collect();
                let seq = RevisionSequence { revisions: labels.clone() };

                let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();

                // Record Pass for all indices <= i, Fail for all indices > i
                // This guarantees a FirstBad outcome
                for idx in 0..n {
                    let class = if idx <= i {
                        ObservationClass::Pass
                    } else {
                        ObservationClass::Fail
                    };
                    session.record(obs(&format!("commit-{idx}"), class)).unwrap();
                }

                let outcome = session.outcome();

                // Must be FirstBad
                match outcome {
                    LocalizationOutcome::FirstBad { last_good, first_bad, .. } => {
                        // Verify last_good has a direct Pass observation
                        let last_good_obs = session.get_observation(&last_good);
                        prop_assert!(last_good_obs.is_some(), "last_good must have a recorded observation");
                        prop_assert_eq!(last_good_obs.unwrap().class, ObservationClass::Pass,
                            "last_good observation must be Pass");

                        // Verify first_bad has a direct Fail observation
                        let first_bad_obs = session.get_observation(&first_bad);
                        prop_assert!(first_bad_obs.is_some(), "first_bad must have a recorded observation");
                        prop_assert_eq!(first_bad_obs.unwrap().class, ObservationClass::Fail,
                            "first_bad observation must be Fail");
                    }
                    other => {
                        prop_assert!(false, "expected FirstBad but got {:?}", other);
                    }
                }
            }

            // Feature: v01-release-train, Property 7: Non-Monotonic Evidence Yields Low Confidence
            // **Validates: Requirement 3.5**
            #[test]
            fn prop_non_monotonic_evidence_yields_low_confidence(
                n in 4usize..=20,
                fail_frac in 0.0f64..1.0,
                pass_frac in 0.0f64..1.0,
                fill_selectors in prop::collection::vec(0u8..2, 20),
            ) {
                // We need fail_idx < pass_idx, both in the interior (1..n-1).
                // Interior range has (n-2) slots: indices 1..=(n-2).
                let interior_size = n - 2; // at least 2 since n >= 4
                // Pick two distinct interior indices
                let raw_a = (fail_frac * interior_size as f64).floor() as usize;
                let raw_a = raw_a.min(interior_size - 1); // clamp to 0..interior_size-1
                let raw_b = (pass_frac * interior_size as f64).floor() as usize;
                let raw_b = raw_b.min(interior_size - 1);

                // Ensure they are distinct; if equal, shift one
                let (a, b) = if raw_a != raw_b {
                    (raw_a, raw_b)
                } else {
                    // shift b up by 1, wrapping
                    (raw_a, (raw_b + 1) % interior_size)
                };

                // Ensure fail_idx < pass_idx (in sequence order)
                let (fail_interior, pass_interior) = if a < b { (a, b) } else { (b, a) };
                let fail_idx = fail_interior + 1; // map back to sequence index
                let pass_idx = pass_interior + 1;

                // Build sequence
                let labels: Vec<CommitId> = (0..n)
                    .map(|idx| CommitId(format!("commit-{idx}")))
                    .collect();
                let seq = RevisionSequence { revisions: labels.clone() };
                let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();

                // Record boundary observations: Pass at first, Fail at last
                session.record(obs("commit-0", ObservationClass::Pass)).unwrap();
                session.record(obs(&format!("commit-{}", n - 1), ObservationClass::Fail)).unwrap();

                // Record non-monotonic pair: Fail at fail_idx, Pass at pass_idx
                session.record(obs(&format!("commit-{fail_idx}"), ObservationClass::Fail)).unwrap();
                session.record(obs(&format!("commit-{pass_idx}"), ObservationClass::Pass)).unwrap();

                // Fill all remaining intermediate commits with Pass or Fail to ensure convergence
                // (no unobserved commits between boundaries)
                for idx in 1..(n - 1) {
                    if idx == fail_idx || idx == pass_idx {
                        continue; // already recorded
                    }
                    let selector = fill_selectors[idx % fill_selectors.len()];
                    let class = if selector % 2 == 0 {
                        ObservationClass::Pass
                    } else {
                        ObservationClass::Fail
                    };
                    session.record(obs(&format!("commit-{idx}"), class)).unwrap();
                }

                let outcome = session.outcome();

                // The outcome must include NonMonotonicEvidence since fail_idx < pass_idx
                match &outcome {
                    LocalizationOutcome::SuspectWindow { reasons, confidence, .. } => {
                        prop_assert!(
                            reasons.contains(&AmbiguityReason::NonMonotonicEvidence),
                            "expected NonMonotonicEvidence in reasons, got {:?}", reasons
                        );
                        prop_assert_eq!(
                            confidence.score, Confidence::low().score,
                            "expected confidence == Confidence::low() ({}), got {}",
                            Confidence::low().score, confidence.score
                        );
                    }
                    other => {
                        // Could also be Inconclusive with NonMonotonicEvidence if boundaries
                        // don't align. Check that NonMonotonicEvidence is present.
                        match other {
                            LocalizationOutcome::Inconclusive { reasons } => {
                                // This can happen if the highest pass is at pass_idx and
                                // there's no fail above it. Still must detect non-monotonic.
                                // But we always have Fail at last commit (n-1) which is > pass_idx,
                                // so this shouldn't happen. Fail if it does.
                                prop_assert!(false,
                                    "unexpected Inconclusive outcome: {:?}", reasons);
                            }
                            LocalizationOutcome::FirstBad { .. } => {
                                prop_assert!(false,
                                    "unexpected FirstBad outcome — non-monotonic evidence should prevent this");
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }

            // Feature: v01-release-train, Property 8: Missing Boundary Yields Inconclusive
            // **Validates: Requirements 3.6, 3.7**
            #[test]
            fn prop_only_pass_yields_inconclusive_missing_fail_boundary(
                n in 3usize..=15,
                num_obs in 1usize..=15,
                selectors in prop::collection::vec(0usize..1000, 15),
            ) {
                let n = n; // sequence length
                let num_obs = num_obs.min(n); // can't observe more than n commits

                let labels: Vec<CommitId> = (0..n)
                    .map(|idx| CommitId(format!("commit-{idx}")))
                    .collect();
                let seq = RevisionSequence { revisions: labels.clone() };
                let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();

                // Pick num_obs distinct indices to observe, all as Pass
                let mut indices: Vec<usize> = (0..n).collect();
                // Fisher-Yates-ish shuffle using selectors
                for i in (1..indices.len()).rev() {
                    let j = selectors[i % selectors.len()] % (i + 1);
                    indices.swap(i, j);
                }
                let chosen: Vec<usize> = indices[..num_obs].to_vec();

                for &idx in &chosen {
                    session.record(obs(&format!("commit-{idx}"), ObservationClass::Pass)).unwrap();
                }

                let outcome = session.outcome();

                match outcome {
                    LocalizationOutcome::Inconclusive { reasons } => {
                        prop_assert!(
                            reasons.contains(&AmbiguityReason::MissingFailBoundary),
                            "expected MissingFailBoundary in reasons, got {:?}", reasons
                        );
                    }
                    other => {
                        prop_assert!(false,
                            "expected Inconclusive but got {:?}", other);
                    }
                }
            }

            // Feature: v01-release-train, Property 8: Missing Boundary Yields Inconclusive
            // **Validates: Requirements 3.6, 3.7**
            #[test]
            fn prop_only_fail_yields_inconclusive_missing_pass_boundary(
                n in 3usize..=15,
                num_obs in 1usize..=15,
                selectors in prop::collection::vec(0usize..1000, 15),
            ) {
                let n = n;
                let num_obs = num_obs.min(n);

                let labels: Vec<CommitId> = (0..n)
                    .map(|idx| CommitId(format!("commit-{idx}")))
                    .collect();
                let seq = RevisionSequence { revisions: labels.clone() };
                let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();

                // Pick num_obs distinct indices to observe, all as Fail
                let mut indices: Vec<usize> = (0..n).collect();
                for i in (1..indices.len()).rev() {
                    let j = selectors[i % selectors.len()] % (i + 1);
                    indices.swap(i, j);
                }
                let chosen: Vec<usize> = indices[..num_obs].to_vec();

                for &idx in &chosen {
                    session.record(obs(&format!("commit-{idx}"), ObservationClass::Fail)).unwrap();
                }

                let outcome = session.outcome();

                match outcome {
                    LocalizationOutcome::Inconclusive { reasons } => {
                        prop_assert!(
                            reasons.contains(&AmbiguityReason::MissingPassBoundary),
                            "expected MissingPassBoundary in reasons, got {:?}", reasons
                        );
                    }
                    other => {
                        prop_assert!(false,
                            "expected Inconclusive but got {:?}", other);
                    }
                }
            }

            // Feature: v01-release-train, Property 21: Monotonic Window Narrowing
            // **Validates: Requirements 11.2**
            //
            // v01-hardening review (task 3.4, Requirements 3.7, 3.8):
            //   Reviewed and confirmed valid. The property correctly verifies that
            //   the pass/fail boundary window never expands between successive
            //   probes when evidence is monotonic (consistent pass→fail transition).
            //   The generator covers sequence sizes 3–20 with a randomly placed
            //   transition point, the window computation mirrors the engine's own
            //   boundary_indices logic, and the final assertion ensures convergence
            //   to window size 1. No changes needed — property is sound as-is.
            #[test]
            fn prop_monotonic_window_narrowing(
                n in 3usize..=20,
                transition_frac in 0.0f64..1.0,
            ) {
                // Pick a monotonic transition point: indices 0..=transition are Pass,
                // indices transition+1..n are Fail. This models a real regression
                // where the predicate is consistent (no non-monotonic evidence).
                let transition = (transition_frac * (n - 1) as f64).floor() as usize;
                let transition = transition.min(n - 2); // ensure at least one Fail after

                let labels: Vec<CommitId> = (0..n)
                    .map(|idx| CommitId(format!("commit-{idx}")))
                    .collect();
                let seq = RevisionSequence { revisions: labels.clone() };
                let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();

                // Helper: compute window size from current observations.
                // Find highest-index Pass and lowest-index Fail above it.
                // Window = fail_idx - pass_idx. If no such pair, window = full sequence length.
                let compute_window = |sess: &LocalizationSession| -> usize {
                    let mut highest_pass: Option<usize> = None;
                    let mut fail_indices: Vec<usize> = Vec::new();
                    for (idx, o) in &sess.observations {
                        match o.class {
                            ObservationClass::Pass => {
                                highest_pass = Some(highest_pass.map_or(*idx, |p: usize| p.max(*idx)));
                            }
                            ObservationClass::Fail => {
                                fail_indices.push(*idx);
                            }
                            _ => {}
                        }
                    }
                    if let Some(hp) = highest_pass {
                        if let Some(lf) = fail_indices.iter().copied().filter(|&f| f > hp).min() {
                            return lf - hp;
                        }
                    }
                    n // full sequence length as fallback
                };

                // Record Pass at first commit and Fail at last commit (establishing boundaries)
                session.record(obs("commit-0", ObservationClass::Pass)).unwrap();
                session.record(obs(&format!("commit-{}", n - 1), ObservationClass::Fail)).unwrap();

                let mut prev_window = compute_window(&session);

                // Follow the binary narrowing order: use next_probe() to pick commits,
                // then assign the correct monotonic class based on the transition point.
                loop {
                    let probe = session.next_probe();
                    if probe.is_none() {
                        break;
                    }
                    let probe_commit = probe.unwrap();
                    let idx = *session.index_by_commit.get(&probe_commit).unwrap();

                    let class = if idx <= transition {
                        ObservationClass::Pass
                    } else {
                        ObservationClass::Fail
                    };
                    session.record(obs(&format!("commit-{idx}"), class)).unwrap();

                    let new_window = compute_window(&session);
                    prop_assert!(
                        new_window <= prev_window,
                        "window expanded from {} to {} after recording commit-{} as {:?}",
                        prev_window, new_window, idx, class
                    );
                    prev_window = new_window;
                }

                // Final window should be exactly 1 (adjacent pass/fail)
                prop_assert_eq!(
                    prev_window, 1,
                    "final window should be 1 (exact boundary found), got {}",
                    prev_window
                );
            }

            // Feature: v01-release-train, Property 22: SuspectWindow Confidence Cap
            // **Validates: Requirement 11.3**
            #[test]
            fn prop_suspect_window_confidence_cap(
                n in 4usize..=20,
                ambig_selectors in prop::collection::vec(0u8..3, 18),
            ) {
                let labels: Vec<CommitId> = (0..n)
                    .map(|idx| CommitId(format!("commit-{idx}")))
                    .collect();
                let seq = RevisionSequence { revisions: labels.clone() };
                let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();

                // Record Pass at first, Fail at last
                session.record(obs("commit-0", ObservationClass::Pass)).unwrap();
                session.record(obs(&format!("commit-{}", n - 1), ObservationClass::Fail)).unwrap();

                // Fill intermediates with Skip or Indeterminate to force SuspectWindow
                for i in 1..(n - 1) {
                    let selector = ambig_selectors[i % ambig_selectors.len()];
                    let class = if selector % 2 == 0 {
                        ObservationClass::Skip
                    } else {
                        ObservationClass::Indeterminate
                    };
                    session.record(obs(&format!("commit-{i}"), class)).unwrap();
                }

                let outcome = session.outcome();

                match outcome {
                    LocalizationOutcome::SuspectWindow { confidence, .. } => {
                        prop_assert!(
                            confidence.score < Confidence::high().score,
                            "SuspectWindow confidence {} must be < {} (Confidence::high().score)",
                            confidence.score, Confidence::high().score
                        );
                    }
                    other => {
                        prop_assert!(false,
                            "expected SuspectWindow but got {:?}", other);
                    }
                }
            }

            // Feature: v01-release-train, Property 6: Ambiguous Observations Yield SuspectWindow
            // **Validates: Requirements 3.3, 3.4**
            #[test]
            fn prop_ambiguous_observations_yield_suspect_window(
                n in 4usize..=20,
                class_selectors in prop::collection::vec(0u8..2, 18),
            ) {
                let labels: Vec<CommitId> = (0..n)
                    .map(|idx| CommitId(format!("commit-{idx}")))
                    .collect();
                let seq = RevisionSequence { revisions: labels.clone() };
                let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();

                // Record Pass at first commit, Fail at last commit
                session.record(obs("commit-0", ObservationClass::Pass)).unwrap();
                session.record(obs(&format!("commit-{}", n - 1), ObservationClass::Fail)).unwrap();

                // For each intermediate commit (indices 1..n-1), assign Skip or Indeterminate.
                // Using only Skip/Indeterminate guarantees no additional Pass/Fail between
                // boundaries, so the boundary pair stays at (0, n-1) and all intermediates
                // are ambiguous.
                let intermediate_count = n - 2;
                let mut has_skip = false;
                let mut has_indeterminate = false;

                for i in 0..intermediate_count {
                    let selector = class_selectors[i % class_selectors.len()];
                    let class = if selector % 2 == 0 {
                        ObservationClass::Skip
                    } else {
                        ObservationClass::Indeterminate
                    };
                    match class {
                        ObservationClass::Skip => has_skip = true,
                        ObservationClass::Indeterminate => has_indeterminate = true,
                        _ => {}
                    }
                    let commit_idx = i + 1;
                    session.record(obs(&format!("commit-{commit_idx}"), class)).unwrap();
                }

                let outcome = session.outcome();

                match outcome {
                    LocalizationOutcome::SuspectWindow { reasons, .. } => {
                        // If any Skip was recorded, SkippedRevision must be in reasons
                        if has_skip {
                            prop_assert!(
                                reasons.contains(&AmbiguityReason::SkippedRevision),
                                "has_skip=true but SkippedRevision not in reasons: {:?}", reasons
                            );
                        }
                        // If any Indeterminate was recorded, IndeterminateRevision must be in reasons
                        if has_indeterminate {
                            prop_assert!(
                                reasons.contains(&AmbiguityReason::IndeterminateRevision),
                                "has_indeterminate=true but IndeterminateRevision not in reasons: {:?}", reasons
                            );
                        }
                    }
                    other => {
                        prop_assert!(false,
                            "expected SuspectWindow but got {:?}", other);
                    }
                }
            }

            // Feature: v01-release-train, Property 23: Observation Order Independence
            // **Validates: Requirement 11.4**
            #[test]
            fn prop_observation_order_independence(
                n in 3usize..=12,
                class_selectors in prop::collection::vec(0u8..4, 12),
                perm_seeds in prop::collection::vec(0usize..1000, 3),
            ) {
                // Build a sequence of n commits
                let labels: Vec<CommitId> = (0..n)
                    .map(|idx| CommitId(format!("commit-{idx}")))
                    .collect();

                // Build observation set: assign a class to every commit
                let observations: Vec<ProbeObservation> = (0..n)
                    .map(|idx| {
                        let class = match class_selectors[idx % class_selectors.len()] % 4 {
                            0 => ObservationClass::Pass,
                            1 => ObservationClass::Fail,
                            2 => ObservationClass::Skip,
                            _ => ObservationClass::Indeterminate,
                        };
                        obs(&format!("commit-{idx}"), class)
                    })
                    .collect();

                // Record in natural order to get the reference outcome
                let seq = RevisionSequence { revisions: labels.clone() };
                let mut session_ref = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
                for o in &observations {
                    session_ref.record(o.clone()).unwrap();
                }
                let reference_outcome = session_ref.outcome();

                // Record in multiple permutation orders and verify same outcome
                for seed in &perm_seeds {
                    let mut permuted = observations.clone();
                    // Simple deterministic permutation using seed
                    let len = permuted.len();
                    for i in (1..len).rev() {
                        let j = (seed.wrapping_mul(i).wrapping_add(7)) % (i + 1);
                        permuted.swap(i, j);
                    }

                    let seq = RevisionSequence { revisions: labels.clone() };
                    let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
                    for o in &permuted {
                        session.record(o.clone()).unwrap();
                    }
                    let outcome = session.outcome();

                    prop_assert_eq!(
                        &outcome, &reference_outcome,
                        "outcome differs for permutation with seed {}: got {:?}, expected {:?}",
                        seed, outcome, reference_outcome
                    );
                }
            }

            // Feature: v01-hardening, Property 25: Max-Probe Exhaustion Produces Explicit Outcome
            // **Validates: Requirements 3.2**
            #[test]
            fn prop_max_probe_exhaustion_produces_explicit_outcome(
                n in 5usize..=30,
                max_probes in 2usize..=5,
                use_all_pass in any::<bool>(),
                selectors in prop::collection::vec(0usize..1000, 30),
            ) {
                // Build a sequence of n commits
                let labels: Vec<CommitId> = (0..n)
                    .map(|idx| CommitId(format!("commit-{idx}")))
                    .collect();
                let seq = RevisionSequence { revisions: labels.clone() };
                let policy = SearchPolicy { max_probes };
                let mut session = LocalizationSession::new(seq, policy).unwrap();

                // Record exactly max_probes observations that do NOT converge.
                // Strategy: record only Pass or only Fail observations so there
                // is never an adjacent pass-fail pair.
                let class = if use_all_pass {
                    ObservationClass::Pass
                } else {
                    ObservationClass::Fail
                };

                // Pick max_probes distinct indices from 0..n using selectors
                let mut indices: Vec<usize> = (0..n).collect();
                for i in (1..indices.len()).rev() {
                    let j = selectors[i % selectors.len()] % (i + 1);
                    indices.swap(i, j);
                }
                let chosen: Vec<usize> = indices[..max_probes].to_vec();

                for &idx in &chosen {
                    session.record(obs(&format!("commit-{idx}"), class)).unwrap();
                }

                // Verify we recorded exactly max_probes observations
                prop_assert_eq!(
                    session.observation_list().len(), max_probes,
                    "should have recorded exactly max_probes={} observations", max_probes
                );

                let outcome = session.outcome();

                // The outcome must include MaxProbesExhausted in its reasons
                let has_max_probes_exhausted = match &outcome {
                    LocalizationOutcome::Inconclusive { reasons } => {
                        reasons.contains(&AmbiguityReason::MaxProbesExhausted)
                    }
                    LocalizationOutcome::SuspectWindow { reasons, .. } => {
                        reasons.contains(&AmbiguityReason::MaxProbesExhausted)
                    }
                    LocalizationOutcome::FirstBad { .. } => false,
                };

                prop_assert!(
                    has_max_probes_exhausted,
                    "outcome should include MaxProbesExhausted, got {:?}", outcome
                );
            }

            // Feature: v01-hardening, Property 26: Observation Sequence Order Preservation
            // **Validates: Requirement 3.3**
            #[test]
            fn prop_observation_sequence_order_preservation(
                n in 2usize..=20,
                seq_indices in prop::collection::vec(0u64..1000, 20),
                class_selectors in prop::collection::vec(0u8..4, 20),
            ) {
                // Build a sequence of n commits
                let labels: Vec<CommitId> = (0..n)
                    .map(|idx| CommitId(format!("commit-{idx}")))
                    .collect();
                let seq = RevisionSequence { revisions: labels.clone() };
                let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();

                // Assign distinct sequence_index values to each observation.
                // Take the first n values from seq_indices and deduplicate by
                // sorting and using the sorted position as a tiebreaker.
                let mut indexed: Vec<(u64, usize)> = seq_indices.iter()
                    .copied()
                    .take(n)
                    .enumerate()
                    .map(|(i, v)| (v, i))
                    .collect();
                // Make indices distinct: sort by (value, original_position) then
                // reassign so each is unique while preserving relative order.
                indexed.sort();
                let distinct_seq_indices: Vec<u64> = indexed.iter()
                    .enumerate()
                    .map(|(rank, _)| rank as u64)
                    .collect();
                // Map back to original commit order
                let mut seq_idx_for_commit = vec![0u64; n];
                for (rank_pos, &(_val, orig_pos)) in indexed.iter().enumerate() {
                    seq_idx_for_commit[orig_pos] = distinct_seq_indices[rank_pos];
                }

                // Record observations in commit order (0..n) with pre-assigned sequence_index
                for i in 0..n {
                    let class = match class_selectors[i % class_selectors.len()] % 4 {
                        0 => ObservationClass::Pass,
                        1 => ObservationClass::Fail,
                        2 => ObservationClass::Skip,
                        _ => ObservationClass::Indeterminate,
                    };
                    session.record(obs_with_seq(
                        &format!("commit-{i}"),
                        class,
                        seq_idx_for_commit[i],
                    )).unwrap();
                }

                // Verify observation_list() returns observations sorted by sequence_index ascending
                let list = session.observation_list();
                prop_assert_eq!(list.len(), n, "observation_list length should equal n={}", n);

                for w in list.windows(2) {
                    prop_assert!(
                        w[0].sequence_index <= w[1].sequence_index,
                        "observation_list not sorted by sequence_index: {} > {}",
                        w[0].sequence_index, w[1].sequence_index
                    );
                }
            }
        }
    }

    // ── Fixture scenarios for localization edge cases (Task 13.6) ────
    // Validates: Requirements 7.5, 7.6, 7.7
    //
    // These use RevisionSequenceBuilder from faultline-fixtures to construct
    // fixture-style scenarios without real Git.

    mod fixture_scenarios {
        use super::*;
        use faultline_fixtures::RevisionSequenceBuilder;

        /// Fixture: Skipped-midpoint
        /// A 3-commit sequence where the midpoint is classified as Skip.
        /// Expected: SuspectWindow with SkippedRevision reason, medium confidence.
        #[test]
        fn fixture_skipped_midpoint_yields_suspect_window() {
            let seq = RevisionSequenceBuilder::with_labels(&["good", "mid", "bad"]);
            let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();

            session.record(obs("good", ObservationClass::Pass)).unwrap();
            session.record(obs("mid", ObservationClass::Skip)).unwrap();
            session.record(obs("bad", ObservationClass::Fail)).unwrap();

            match session.outcome() {
                LocalizationOutcome::SuspectWindow {
                    lower_bound_exclusive,
                    upper_bound_inclusive,
                    confidence,
                    reasons,
                } => {
                    assert_eq!(lower_bound_exclusive.0, "good");
                    assert_eq!(upper_bound_inclusive.0, "bad");
                    assert!(reasons.contains(&AmbiguityReason::SkippedRevision));
                    assert_eq!(confidence, Confidence::medium());
                }
                other => panic!("expected SuspectWindow, got: {other:?}"),
            }
        }

        /// Fixture: Timed-out-midpoint
        /// A 3-commit sequence where the midpoint is classified as Indeterminate.
        /// Expected: SuspectWindow with IndeterminateRevision reason.
        #[test]
        fn fixture_timed_out_midpoint_yields_suspect_window() {
            let seq = RevisionSequenceBuilder::with_labels(&["good", "mid", "bad"]);
            let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();

            session.record(obs("good", ObservationClass::Pass)).unwrap();
            session
                .record(obs("mid", ObservationClass::Indeterminate))
                .unwrap();
            session.record(obs("bad", ObservationClass::Fail)).unwrap();

            match session.outcome() {
                LocalizationOutcome::SuspectWindow {
                    lower_bound_exclusive,
                    upper_bound_inclusive,
                    confidence,
                    reasons,
                } => {
                    assert_eq!(lower_bound_exclusive.0, "good");
                    assert_eq!(upper_bound_inclusive.0, "bad");
                    assert!(reasons.contains(&AmbiguityReason::IndeterminateRevision));
                    assert!(confidence.score < Confidence::high().score);
                }
                other => panic!("expected SuspectWindow, got: {other:?}"),
            }
        }

        /// Fixture: Non-monotonic evidence
        /// A 4-commit sequence where a Fail appears before a Pass (Fail at idx 1,
        /// Pass at idx 2), violating the expected monotonic pass→fail transition.
        /// Expected: SuspectWindow with NonMonotonicEvidence, low confidence.
        #[test]
        fn fixture_non_monotonic_yields_low_confidence() {
            let seq = RevisionSequenceBuilder::with_labels(&["a", "b", "c", "d"]);
            let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();

            // b (idx 1) = Fail, c (idx 2) = Pass → non-monotonic
            session.record(obs("b", ObservationClass::Fail)).unwrap();
            session.record(obs("c", ObservationClass::Pass)).unwrap();
            session.record(obs("d", ObservationClass::Fail)).unwrap();

            match session.outcome() {
                LocalizationOutcome::SuspectWindow {
                    confidence,
                    reasons,
                    ..
                } => {
                    assert!(reasons.contains(&AmbiguityReason::NonMonotonicEvidence));
                    assert_eq!(confidence, Confidence::low());
                }
                other => panic!("expected SuspectWindow with NonMonotonicEvidence, got: {other:?}"),
            }
        }
    }
}
