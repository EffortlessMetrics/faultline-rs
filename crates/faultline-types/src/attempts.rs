//! Conservative aggregation of retained executions. Process metadata remains
//! on original attempts, never borrowed to describe an aggregate decision.
use crate::{
    CommitId, FaultlineError, FlakeSignal, ProbeObservation, Result, compute_flake_signal,
};
use faultline_codes::ObservationClass;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProbeAggregate {
    pub commit: CommitId,
    /// Run-local immutable attempt sequence numbers, in execution order.
    pub attempt_ids: Vec<u64>,
    pub class: ObservationClass,
    pub counts: FlakeSignal,
    pub decision_rule: String,
}

pub fn aggregate_attempts(attempts: &[ProbeObservation]) -> Result<ProbeAggregate> {
    let first = attempts
        .first()
        .ok_or_else(|| FaultlineError::Domain("cannot aggregate no attempts".into()))?;
    let mut identities = BTreeSet::new();
    for attempt in attempts {
        if attempt.commit != first.commit
            || attempt.kind != first.kind
            || attempt.flake_signal.is_some()
        {
            return Err(FaultlineError::Domain(
                "aggregate requires raw attempts for one commit and probe kind".into(),
            ));
        }
        if !identities.insert(attempt.sequence_index) {
            return Err(FaultlineError::Domain("duplicate attempt identity".into()));
        }
    }
    u32::try_from(attempts.len())
        .map_err(|_| FaultlineError::Domain("too many attempts".into()))?;
    let classes: Vec<_> = attempts.iter().map(|a| a.class).collect();
    let counts = compute_flake_signal(&classes, 1.0);
    let class = if counts.is_stable {
        first.class
    } else {
        ObservationClass::Indeterminate
    };
    Ok(ProbeAggregate {
        commit: first.commit.clone(),
        attempt_ids: attempts.iter().map(|a| a.sequence_index).collect(),
        class,
        counts,
        decision_rule: "unanimous_complete_v1".into(),
    })
}

/// Compatibility input for search, not a raw process result. Multi-attempt
/// summaries contain no invented exit code, duration, output or working directory.
pub fn aggregate_projection(attempts: &[ProbeObservation]) -> Result<ProbeObservation> {
    let aggregate = aggregate_attempts(attempts)?;
    let last = attempts
        .last()
        .ok_or_else(|| FaultlineError::Domain("missing attempt".into()))?;
    if attempts.len() == 1 {
        return Ok(last.clone());
    }
    Ok(ProbeObservation {
        commit: aggregate.commit,
        class: aggregate.class,
        kind: last.kind,
        exit_code: None,
        timed_out: false,
        duration_ms: 0,
        stdout: String::new(),
        stderr: String::new(),
        sequence_index: last.sequence_index,
        signal_number: None,
        probe_command: String::new(),
        working_dir: String::new(),
        flake_signal: Some(aggregate.counts),
    })
}
