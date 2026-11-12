use faultline_integration::{
    ChangeUnit, IntegrationSequence, IntegrationSnapshotId, IntegrationState,
    IntegrationStateId, StateEquivalence, StateProvenance, TreeId,
};
use faultline_queue::{
    DiagnosisInput, DiagnosisPolicy, EvidenceSource, LaunchRecord, PhaseConclusion,
    PhaseReceipt, SCHEMA_VERSION, TargetIdentity,
};
use faultline_types::CommitId;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

pub fn packet(count: usize) -> TestResult<DiagnosisInput> {
    let snapshot = IntegrationSnapshotId("test-incident".into());
    let mut changes = Vec::new();
    let mut states = Vec::new();
    for ordinal in 0..=count {
        let transition = if ordinal == 0 {
            None
        } else {
            Some(ChangeUnit::PullRequest {
                provider: "synthetic".into(),
                repository: "fixture/queue".into(),
                number: u64::try_from(ordinal)?,
                head: CommitId(format!("{:040x}", ordinal + 100)),
            })
        };
        if let Some(change) = &transition {
            changes.push(change.clone());
        }
        states.push(IntegrationState {
            id: IntegrationStateId(format!("S{ordinal}")), ordinal,
            commit: CommitId(format!("{:040x}", ordinal + 1)),
            tree: TreeId(format!("{:040x}", ordinal + 1000)),
            included_changes: changes.clone(), transition,
            equivalence: StateEquivalence::ExactCommit,
            provenance: StateProvenance::Provider {
                provider: "synthetic".into(), snapshot: snapshot.clone(),
            },
        });
    }
    Ok(DiagnosisInput {
        schema_version: SCHEMA_VERSION.into(),
        sequence: IntegrationSequence::try_new("synthetic", snapshot, states)?,
        target: TargetIdentity {
            phase: "tests".into(), failure_signature: "assert:test_a".into(),
            predicate_id: "predicate-v1".into(), context_id: "linux-rust-pinned-v1".into(),
        },
        policy: DiagnosisPolicy {
            max_launches: 5, minimum_attempts: 1, assume_monotonic: true,
            required_equivalence: StateEquivalence::ExactCommit,
        },
        launches: Vec::new(), receipts: Vec::new(),
    })
}

pub fn receipt(
    input: &DiagnosisInput,
    ordinal: usize,
    id: &str,
    passing: bool,
    source: EvidenceSource,
) -> TestResult<PhaseReceipt> {
    let state = input.sequence.states.get(ordinal).ok_or("fixture ordinal")?;
    Ok(PhaseReceipt {
        attempt_id: id.into(), snapshot_id: input.sequence.snapshot_id.0.clone(),
        state_id: state.id.0.clone(), commit: state.commit.0.clone(), tree: state.tree.0.clone(),
        phase: input.target.phase.clone(), predicate_id: input.target.predicate_id.clone(),
        context_id: input.target.context_id.clone(), source,
        conclusion: if passing { PhaseConclusion::Passed } else { PhaseConclusion::Failed },
        exit_code: Some(if passing { 0 } else { 1 }),
        failure_signature: if passing { None } else { Some(input.target.failure_signature.clone()) },
        log_id: format!("log:{id}"),
    })
}

pub fn add(
    input: &mut DiagnosisInput,
    ordinal: usize,
    id: &str,
    passing: bool,
    source: EvidenceSource,
) -> TestResult<()> {
    let observation = receipt(input, ordinal, id, passing, source)?;
    if source == EvidenceSource::Native {
        input.launches.push(LaunchRecord {
            attempt_id: id.into(), state_id: observation.state_id.clone(),
            commit: observation.commit.clone(), tree: observation.tree.clone(),
        });
    }
    input.receipts.push(observation);
    Ok(())
}

pub fn endpoints(input: &mut DiagnosisInput) -> TestResult<()> {
    add(input, 0, "base", true, EvidenceSource::Provider)?;
    add(input, input.sequence.len() - 1, "head", false, EvidenceSource::Provider)
}
