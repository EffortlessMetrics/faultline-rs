use faultline_codes::AmbiguityReason;
use faultline_integration::{
    ChangeUnit, IntegrationError, IntegrationOutcome, IntegrationSequence, IntegrationSnapshotId,
    IntegrationState, IntegrationStateId, LocalizationBasis, StateEquivalence, StateProvenance,
    TreeId,
};
use faultline_types::{CommitId, Confidence, LocalizationOutcome};
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn sequence(count: usize) -> Result<IntegrationSequence, Box<dyn std::error::Error>> {
    let snapshot = IntegrationSnapshotId("incident".into());
    let mut changes = Vec::new();
    let mut states = Vec::new();
    for ordinal in 0..=count {
        let transition = if ordinal == 0 {
            None
        } else {
            Some(ChangeUnit::PullRequest {
                provider: "github".into(),
                repository: "owner/repo".into(),
                number: u64::try_from(ordinal)?,
                head: CommitId(format!("pr-{ordinal}")),
            })
        };
        if let Some(change) = &transition {
            changes.push(change.clone());
        }
        states.push(IntegrationState {
            id: IntegrationStateId(format!("S{ordinal}")),
            ordinal,
            commit: CommitId(format!("commit-{ordinal}")),
            tree: TreeId(format!("tree-{ordinal}")),
            included_changes: changes.clone(),
            transition,
            equivalence: StateEquivalence::ExactCommit,
            provenance: StateProvenance::Provider {
                provider: "github".into(),
                snapshot: snapshot.clone(),
            },
        });
    }
    Ok(IntegrationSequence::try_new("github", snapshot, states)?)
}

fn first(lower: usize, upper: usize) -> LocalizationOutcome {
    LocalizationOutcome::FirstBad {
        last_good: CommitId(format!("commit-{lower}")),
        first_bad: CommitId(format!("commit-{upper}")),
        confidence: Confidence::high(),
    }
}

#[test]
fn explicit_basis_is_retained_on_every_adjacent_transition() -> TestResult {
    let sequence = sequence(20)?;
    for ordinal in 1..=20 {
        let mapped = sequence.map_localization_outcome_with_basis(
            &first(ordinal - 1, ordinal),
            StateEquivalence::ExactCommit,
            LocalizationBasis::MonotonicTargetAssumed,
        )?;
        match mapped {
            IntegrationOutcome::FirstFailingTransition {
                first_failing,
                transition: ChangeUnit::PullRequest { number, .. },
                basis,
                ..
            } => {
                assert_eq!(first_failing.ordinal, ordinal);
                assert_eq!(number, u64::try_from(ordinal)?);
                assert_eq!(basis, LocalizationBasis::MonotonicTargetAssumed);
            }
            _ => return Err("lost transition identity".into()),
        }
    }
    Ok(())
}

#[test]
fn rejects_cross_snapshot_or_cross_provider_provenance() -> TestResult {
    for bad_provider in [false, true] {
        let mut sequence = sequence(2)?;
        let state = sequence.states.get_mut(1).ok_or("state")?;
        if let StateProvenance::Provider { provider, snapshot } = &mut state.provenance {
            if bad_provider {
                *provider = "other".into();
            } else {
                snapshot.0 = "other".into();
            }
        }
        assert!(matches!(
            sequence.validate(),
            Err(IntegrationError::ProvenanceMismatch { ordinal: 1 })
        ));
    }
    Ok(())
}

#[test]
fn reconstructed_tree_requires_a_same_prefix_witness() -> TestResult {
    let mut sequence = sequence(2)?;
    let snapshot = sequence.snapshot_id.clone();
    let state = sequence.states.get_mut(1).ok_or("state")?;
    state.equivalence = StateEquivalence::ExactTree;
    state.provenance = StateProvenance::Reconstructed {
        builder: "git-builder".into(),
        source_snapshot: snapshot,
        provider_tree: None,
    };
    assert!(matches!(
        sequence.validate(),
        Err(IntegrationError::MissingEquivalenceWitness { .. })
    ));
    let state = sequence.states.get_mut(1).ok_or("state")?;
    if let StateProvenance::Reconstructed { provider_tree, .. } = &mut state.provenance {
        *provider_tree = Some(TreeId("tree-2".into()));
    }
    assert!(
        sequence.validate().is_err(),
        "final-group tree is not a prefix witness"
    );
    let state = sequence.states.get_mut(1).ok_or("state")?;
    if let StateProvenance::Reconstructed { provider_tree, .. } = &mut state.provenance {
        *provider_tree = Some(TreeId("tree-1".into()));
    }
    sequence.validate()?;
    Ok(())
}

#[test]
fn reconstructed_provenance_cannot_assert_an_exact_provider_commit() -> TestResult {
    let mut sequence = sequence(1)?;
    let snapshot = sequence.snapshot_id.clone();
    let state = sequence.states.get_mut(1).ok_or("state")?;
    state.provenance = StateProvenance::Reconstructed {
        builder: "builder".into(),
        source_snapshot: snapshot,
        provider_tree: Some(state.tree.clone()),
    };
    assert!(sequence.validate().is_err());
    Ok(())
}

#[test]
fn repeated_pr_with_a_different_head_is_still_a_duplicate() -> TestResult {
    let mut sequence = sequence(2)?;
    let state = sequence.states.get_mut(2).ok_or("state")?;
    let mut changed = state.transition.clone().ok_or("transition")?;
    if let ChangeUnit::PullRequest { number, .. } = &mut changed {
        *number = 1;
    }
    state.transition = Some(changed.clone());
    *state.included_changes.last_mut().ok_or("change")? = changed;
    assert!(matches!(
        sequence.validate(),
        Err(IntegrationError::RepeatedTransition { .. })
    ));
    Ok(())
}

#[test]
fn rejects_invalid_change_identity_and_cross_repository_membership() -> TestResult {
    for case in 0..4 {
        let mut sequence = sequence(2)?;
        let state = sequence.states.get_mut(2).ok_or("state")?;
        let mut change = state.transition.clone().ok_or("transition")?;
        if let ChangeUnit::PullRequest {
            provider,
            repository,
            number,
            head,
        } = &mut change
        {
            match case {
                0 => *number = 0,
                1 => head.0.clear(),
                2 => *provider = "wrong".into(),
                _ => *repository = "other/repo".into(),
            }
        }
        state.transition = Some(change.clone());
        *state.included_changes.last_mut().ok_or("change")? = change;
        assert!(sequence.validate().is_err());
    }
    Ok(())
}

#[test]
fn rejects_non_cumulative_order_and_duplicate_state_identity() -> TestResult {
    let mut input = sequence(2)?;
    input
        .states
        .get_mut(2)
        .ok_or("state")?
        .included_changes
        .reverse();
    assert!(input.validate().is_err());
    let mut input = sequence(2)?;
    input.states.get_mut(2).ok_or("state")?.id = IntegrationStateId("S1".into());
    assert!(input.validate().is_err());
    Ok(())
}

#[test]
fn no_op_transition_may_have_an_identical_tree() -> TestResult {
    let mut input = sequence(2)?;
    for state in &mut input.states {
        state.tree = TreeId("identical-tree".into());
    }
    input.validate()?;
    assert_eq!(input.revision_sequence()?.len(), 3);
    Ok(())
}

#[test]
fn weak_minimum_does_not_upgrade_patch_equivalence() -> TestResult {
    let mut input = sequence(1)?;
    input.states.get_mut(1).ok_or("state")?.equivalence = StateEquivalence::PatchEquivalent;
    let mapped = input.map_localization_outcome_with_basis(
        &first(0, 1),
        StateEquivalence::Unverified,
        LocalizationBasis::MonotonicTargetAssumed,
    )?;
    assert!(matches!(
        mapped,
        IntegrationOutcome::SuspectTransitions { .. }
    ));
    Ok(())
}

#[test]
fn foreign_reverse_and_nonadjacent_boundaries_never_become_exact() -> TestResult {
    let input = sequence(3)?;
    for (lower, upper) in [(2, 1), (1, 1), (0, 4)] {
        assert!(
            input
                .map_localization_outcome_with_basis(
                    &first(lower, upper),
                    StateEquivalence::ExactCommit,
                    LocalizationBasis::MonotonicTargetAssumed
                )
                .is_err()
        );
    }
    assert!(matches!(
        input.map_localization_outcome_with_basis(
            &first(0, 3),
            StateEquivalence::ExactCommit,
            LocalizationBasis::MonotonicTargetAssumed
        )?,
        IntegrationOutcome::SuspectTransitions { .. }
    ));
    Ok(())
}

#[test]
fn non_monotonic_and_inconclusive_outcomes_preserve_their_meaning() -> TestResult {
    let input = sequence(3)?;
    let outcome = LocalizationOutcome::Inconclusive {
        reasons: vec![AmbiguityReason::NonMonotonicEvidence],
    };
    assert!(matches!(
        input.map_localization_outcome(&outcome, StateEquivalence::ExactCommit)?,
        IntegrationOutcome::NonMonotonic { .. }
    ));
    let outcome = LocalizationOutcome::Inconclusive {
        reasons: vec![AmbiguityReason::MissingPassBoundary],
    };
    assert!(matches!(
        input.map_localization_outcome(&outcome, StateEquivalence::ExactCommit)?,
        IntegrationOutcome::Inconclusive { .. }
    ));
    Ok(())
}

#[test]
fn deserialized_invalid_sequences_are_revalidated_at_use() -> TestResult {
    let input = sequence(2)?;
    let bytes = serde_json::to_vec(&input)?;
    let mut input: IntegrationSequence = serde_json::from_slice(&bytes)?;
    input.states.get_mut(1).ok_or("state")?.ordinal = 9;
    assert!(input.revision_sequence().is_err());
    assert!(
        input
            .map_localization_outcome(&first(0, 1), StateEquivalence::ExactCommit)
            .is_err()
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn arbitrary_valid_prefixes_map_only_the_introduced_pr(count in 1usize..=64, seed in 0usize..64) {
        let input = sequence(count).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let ordinal = seed % count + 1;
        let mapped = input.map_localization_outcome_with_basis(&first(ordinal - 1, ordinal), StateEquivalence::ExactCommit, LocalizationBasis::MonotonicTargetAssumed).map_err(|e| TestCaseError::fail(e.to_string()))?;
        if let IntegrationOutcome::FirstFailingTransition { first_failing, .. } = mapped {
            prop_assert_eq!(first_failing.ordinal, ordinal);
        } else {
            prop_assert!(false, "valid adjacent transition was not mapped");
        }
    }
}
