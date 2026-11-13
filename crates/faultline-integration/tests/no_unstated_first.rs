use faultline_integration::{
    ChangeUnit, IntegrationOutcome, IntegrationSequence, IntegrationSnapshotId, IntegrationState,
    IntegrationStateId, StateEquivalence, StateProvenance, TreeId,
};
use faultline_types::{CommitId, Confidence, LocalizationOutcome};

// This regression test also compiles against the pre-fix domain API.
#[test]
fn default_mapping_never_assumes_monotonicity() -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = IntegrationSnapshotId("incident".into());
    let change = ChangeUnit::PullRequest {
        provider: "github".into(),
        repository: "example/repo".into(),
        number: 14,
        head: CommitId("pr-head".into()),
    };
    let mut states = Vec::new();
    for ordinal in 0..=1 {
        states.push(IntegrationState {
            id: IntegrationStateId(format!("S{ordinal}")),
            ordinal,
            commit: CommitId(format!("commit-{ordinal}")),
            tree: TreeId(format!("tree-{ordinal}")),
            included_changes: if ordinal == 0 {
                vec![]
            } else {
                vec![change.clone()]
            },
            transition: if ordinal == 0 {
                None
            } else {
                Some(change.clone())
            },
            equivalence: StateEquivalence::ExactCommit,
            provenance: StateProvenance::Provider {
                provider: "github".into(),
                snapshot: snapshot.clone(),
            },
        });
    }
    let sequence = IntegrationSequence::try_new("github", snapshot, states)?;
    let outcome = sequence.map_localization_outcome(
        &LocalizationOutcome::FirstBad {
            last_good: CommitId("commit-0".into()),
            first_bad: CommitId("commit-1".into()),
            confidence: Confidence::high(),
        },
        StateEquivalence::ExactCommit,
    )?;
    assert!(
        matches!(outcome, IntegrationOutcome::SuspectTransitions { .. }),
        "an unstated assumption must not become a first-transition claim"
    );
    Ok(())
}
