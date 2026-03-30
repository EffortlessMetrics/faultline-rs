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
        let (pass_boundary, fail_boundary, non_monotonic) = self.boundaries_and_reasons();

        // Missing boundaries → Inconclusive
        let Some(lower) = pass_boundary else {
            return LocalizationOutcome::Inconclusive {
                reasons: vec![AmbiguityReason::MissingPassBoundary],
            };
        };

        let Some(upper) = fail_boundary else {
            let mut reasons = vec![AmbiguityReason::MissingFailBoundary];
            if non_monotonic {
                reasons.push(AmbiguityReason::NonMonotonicEvidence);
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
        }
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
    fn observation_list_returns_all_in_index_order() {
        let seq = make_seq(&["a", "b", "c"]);
        let mut session = LocalizationSession::new(seq, SearchPolicy::default()).unwrap();
        session.record(obs("c", ObservationClass::Fail)).unwrap();
        session.record(obs("a", ObservationClass::Pass)).unwrap();
        let list = session.observation_list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].commit.0, "a");
        assert_eq!(list[1].commit.0, "c");
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
        }
    }
}
