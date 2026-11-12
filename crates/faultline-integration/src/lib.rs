//! Pure cumulative-state identities and honest integration-transition mapping.
//!
//! No Git, process, filesystem, network or provider I/O belongs here. Adapters
//! establish object provenance; this crate checks internal consistency. A
//! sampled boundary is not the *first* boundary without an explicit search
//! basis. The default mapping therefore produces a suspect result.

#![forbid(unsafe_code)]

use faultline_codes::AmbiguityReason;
use faultline_types::{CommitId, Confidence, LocalizationOutcome, RevisionSequence};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, IntegrationError>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationSnapshotId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationStateId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct TreeId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum ChangeUnit {
    PullRequest {
        provider: String,
        repository: String,
        number: u64,
        head: CommitId,
    },
    Commit {
        repository: String,
        commit: CommitId,
    },
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum StateEquivalence {
    Unverified,
    PatchEquivalent,
    ExactTree,
    ExactCommit,
}

impl StateEquivalence {
    #[must_use]
    pub fn satisfies(self, required: Self) -> bool {
        self >= required
    }
}

/// The caller must establish this basis from its policy or complete evidence.
/// A monotonicity assumption is recorded, never silently treated as proof.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum LocalizationBasis {
    #[default]
    Unspecified,
    MonotonicTargetAssumed,
    CompletePrefixObserved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum StateProvenance {
    Provider {
        provider: String,
        snapshot: IntegrationSnapshotId,
    },
    Reconstructed {
        builder: String,
        source_snapshot: IntegrationSnapshotId,
        /// Independently retained tree for this same cumulative prefix.
        /// Matching the final group tree does not witness an earlier prefix.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_tree: Option<TreeId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntegrationState {
    pub id: IntegrationStateId,
    pub ordinal: usize,
    pub commit: CommitId,
    pub tree: TreeId,
    pub included_changes: Vec<ChangeUnit>,
    pub transition: Option<ChangeUnit>,
    pub equivalence: StateEquivalence,
    pub provenance: StateProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationStateRef {
    pub id: IntegrationStateId,
    pub ordinal: usize,
    pub commit: CommitId,
    pub tree: TreeId,
    pub equivalence: StateEquivalence,
}

impl From<&IntegrationState> for IntegrationStateRef {
    fn from(state: &IntegrationState) -> Self {
        Self {
            id: state.id.clone(),
            ordinal: state.ordinal,
            commit: state.commit.clone(),
            tree: state.tree.clone(),
            equivalence: state.equivalence,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntegrationSequence {
    pub provider: String,
    pub snapshot_id: IntegrationSnapshotId,
    pub states: Vec<IntegrationState>,
}

impl IntegrationSequence {
    pub fn try_new(
        provider: impl Into<String>,
        snapshot_id: IntegrationSnapshotId,
        states: Vec<IntegrationState>,
    ) -> Result<Self> {
        let sequence = Self {
            provider: provider.into(),
            snapshot_id,
            states,
        };
        sequence.validate()?;
        Ok(sequence)
    }

    pub fn validate(&self) -> Result<()> {
        if self.provider.trim().is_empty() {
            return Err(IntegrationError::EmptyProvider);
        }
        if self.snapshot_id.0.trim().is_empty() {
            return Err(IntegrationError::EmptySnapshotId);
        }
        if self.states.len() < 2 {
            return Err(IntegrationError::SequenceTooShort {
                actual: self.states.len(),
            });
        }
        let mut state_ids = HashSet::new();
        let mut commits = HashSet::new();
        let mut changes = HashSet::new();
        let mut repository: Option<&str> = None;
        let mut previous: Option<&IntegrationState> = None;
        for (ordinal, state) in self.states.iter().enumerate() {
            if state.id.0.trim().is_empty() {
                return Err(IntegrationError::EmptyStateId { ordinal });
            }
            if state.commit.0.trim().is_empty() {
                return Err(IntegrationError::EmptyCommit { ordinal });
            }
            if state.tree.0.trim().is_empty() {
                return Err(IntegrationError::EmptyTree { ordinal });
            }
            if state.ordinal != ordinal {
                return Err(IntegrationError::OrdinalMismatch {
                    expected: ordinal,
                    actual: state.ordinal,
                });
            }
            if !state_ids.insert(&state.id) {
                return Err(IntegrationError::DuplicateStateId {
                    state_id: state.id.0.clone(),
                });
            }
            if !commits.insert(&state.commit) {
                return Err(IntegrationError::DuplicateCommit {
                    commit: state.commit.0.clone(),
                });
            }
            self.validate_provenance(state)?;
            if let Some(prior) = previous {
                let transition = state
                    .transition
                    .as_ref()
                    .ok_or(IntegrationError::MissingTransition { ordinal })?;
                let (repo, identity) = self.change_identity(transition, ordinal)?;
                if repository.is_some_and(|expected| expected != repo) {
                    return Err(IntegrationError::InvalidChange { ordinal });
                }
                repository = Some(repo);
                if !changes.insert(identity) {
                    return Err(IntegrationError::RepeatedTransition { ordinal });
                }
                if state.included_changes.len() != prior.included_changes.len() + 1
                    || !state.included_changes.starts_with(&prior.included_changes)
                    || state.included_changes.last() != Some(transition)
                {
                    return Err(IntegrationError::NonCumulativeState { ordinal });
                }
            } else {
                if state.transition.is_some() {
                    return Err(IntegrationError::BaseHasTransition);
                }
                if !state.included_changes.is_empty() {
                    return Err(IntegrationError::BaseHasIncludedChanges);
                }
            }
            previous = Some(state);
        }
        Ok(())
    }

    fn validate_provenance(&self, state: &IntegrationState) -> Result<()> {
        let ordinal = state.ordinal;
        match &state.provenance {
            StateProvenance::Provider { provider, snapshot } => {
                if provider != &self.provider || snapshot != &self.snapshot_id {
                    return Err(IntegrationError::ProvenanceMismatch { ordinal });
                }
            }
            StateProvenance::Reconstructed {
                builder,
                source_snapshot,
                provider_tree,
            } => {
                if builder.trim().is_empty() || source_snapshot != &self.snapshot_id {
                    return Err(IntegrationError::ProvenanceMismatch { ordinal });
                }
                if state.equivalence == StateEquivalence::ExactCommit
                    || (state.equivalence == StateEquivalence::ExactTree
                        && provider_tree.as_ref() != Some(&state.tree))
                {
                    return Err(IntegrationError::MissingEquivalenceWitness { ordinal });
                }
            }
        }
        Ok(())
    }

    fn change_identity<'a>(
        &self,
        change: &'a ChangeUnit,
        ordinal: usize,
    ) -> Result<(&'a str, String)> {
        match change {
            ChangeUnit::PullRequest {
                provider,
                repository,
                number,
                head,
            } if provider == &self.provider
                && !repository.trim().is_empty()
                && *number > 0
                && !head.0.trim().is_empty() =>
            {
                // Head changes do not make a second occurrence of the same PR valid.
                Ok((repository, format!("pr:{number}")))
            }
            ChangeUnit::Commit { repository, commit }
                if !repository.trim().is_empty() && !commit.0.trim().is_empty() =>
            {
                Ok((repository, format!("commit:{}", commit.0)))
            }
            _ => Err(IntegrationError::InvalidChange { ordinal }),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.states.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    pub fn revision_sequence(&self) -> Result<RevisionSequence> {
        self.validate()?;
        Ok(RevisionSequence {
            revisions: self
                .states
                .iter()
                .map(|state| state.commit.clone())
                .collect(),
        })
    }

    /// Conservative compatibility entry point: no unstated monotonicity assumption.
    pub fn map_localization_outcome(
        &self,
        outcome: &LocalizationOutcome,
        required_equivalence: StateEquivalence,
    ) -> Result<IntegrationOutcome> {
        self.map_localization_outcome_with_basis(
            outcome,
            required_equivalence,
            LocalizationBasis::Unspecified,
        )
    }

    pub fn map_localization_outcome_with_basis(
        &self,
        outcome: &LocalizationOutcome,
        required_equivalence: StateEquivalence,
        basis: LocalizationBasis,
    ) -> Result<IntegrationOutcome> {
        self.validate()?;
        let (lower, upper, confidence, core_reasons, nominal_first) = match outcome {
            LocalizationOutcome::FirstBad {
                last_good,
                first_bad,
                confidence,
            } => (last_good, first_bad, confidence, &[][..], true),
            LocalizationOutcome::SuspectWindow {
                lower_bound_exclusive,
                upper_bound_inclusive,
                confidence,
                reasons,
            } => (
                lower_bound_exclusive,
                upper_bound_inclusive,
                confidence,
                reasons.as_slice(),
                false,
            ),
            LocalizationOutcome::Inconclusive { reasons } => {
                let mapped = Self::map_reasons(reasons);
                return Ok(if Self::non_monotonic(reasons) {
                    IntegrationOutcome::NonMonotonic {
                        lower_bound_exclusive: None,
                        upper_bound_inclusive: None,
                        transitions: Vec::new(),
                        confidence: None,
                        reasons: mapped,
                    }
                } else {
                    IntegrationOutcome::Inconclusive { reasons: mapped }
                });
            }
        };
        let lower_state = self.boundary(lower)?;
        let upper_state = self.boundary(upper)?;
        if upper_state.ordinal <= lower_state.ordinal {
            return Err(IntegrationError::InvalidBoundaryOrder {
                lower_ordinal: lower_state.ordinal,
                upper_ordinal: upper_state.ordinal,
            });
        }
        let transitions = self
            .states
            .iter()
            .filter(|s| s.ordinal > lower_state.ordinal && s.ordinal <= upper_state.ordinal)
            .filter_map(|s| s.transition.clone())
            .collect();
        let mut reasons = Self::map_reasons(core_reasons);
        // A weak predicate minimum does not authorize an exact provider-bound claim.
        let minimum = required_equivalence.max(StateEquivalence::ExactTree);
        for state in self
            .states
            .iter()
            .filter(|s| s.ordinal >= lower_state.ordinal && s.ordinal <= upper_state.ordinal)
        {
            if !state.equivalence.satisfies(minimum) {
                reasons.push(IntegrationAmbiguityReason::InsufficientStateEquivalence {
                    ordinal: state.ordinal,
                    actual: state.equivalence,
                    required: minimum,
                });
            }
        }
        if Self::non_monotonic(core_reasons) {
            return Ok(IntegrationOutcome::NonMonotonic {
                lower_bound_exclusive: Some(lower_state.into()),
                upper_bound_inclusive: Some(upper_state.into()),
                transitions,
                confidence: Some(confidence.clone()),
                reasons,
            });
        }
        if nominal_first {
            if basis == LocalizationBasis::Unspecified {
                reasons.push(IntegrationAmbiguityReason::MonotonicityNotEstablished);
            }
            if upper_state.ordinal != lower_state.ordinal + 1 {
                reasons.push(IntegrationAmbiguityReason::NonAdjacentBoundary {
                    lower_ordinal: lower_state.ordinal,
                    upper_ordinal: upper_state.ordinal,
                });
            }
            if reasons.is_empty() {
                let transition =
                    upper_state
                        .transition
                        .clone()
                        .ok_or(IntegrationError::MissingTransition {
                            ordinal: upper_state.ordinal,
                        })?;
                return Ok(IntegrationOutcome::FirstFailingTransition {
                    last_passing: lower_state.into(),
                    first_failing: upper_state.into(),
                    transition,
                    boundary_equivalence: lower_state.equivalence.min(upper_state.equivalence),
                    confidence: confidence.clone(),
                    basis,
                });
            }
        }
        Ok(IntegrationOutcome::SuspectTransitions {
            lower_bound_exclusive: lower_state.into(),
            upper_bound_inclusive: upper_state.into(),
            transitions,
            confidence: confidence.clone(),
            reasons,
        })
    }

    fn boundary(&self, commit: &CommitId) -> Result<&IntegrationState> {
        self.states
            .iter()
            .find(|state| &state.commit == commit)
            .ok_or_else(|| IntegrationError::UnknownBoundaryCommit {
                commit: commit.0.clone(),
            })
    }

    fn non_monotonic(reasons: &[AmbiguityReason]) -> bool {
        reasons.contains(&AmbiguityReason::NonMonotonicEvidence)
    }

    fn map_reasons(reasons: &[AmbiguityReason]) -> Vec<IntegrationAmbiguityReason> {
        reasons
            .iter()
            .cloned()
            .map(IntegrationAmbiguityReason::Localization)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum IntegrationAmbiguityReason {
    Localization(AmbiguityReason),
    MonotonicityNotEstablished,
    InsufficientStateEquivalence {
        ordinal: usize,
        actual: StateEquivalence,
        required: StateEquivalence,
    },
    NonAdjacentBoundary {
        lower_ordinal: usize,
        upper_ordinal: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum IntegrationOutcome {
    FirstFailingTransition {
        last_passing: IntegrationStateRef,
        first_failing: IntegrationStateRef,
        transition: ChangeUnit,
        boundary_equivalence: StateEquivalence,
        confidence: Confidence,
        basis: LocalizationBasis,
    },
    SuspectTransitions {
        lower_bound_exclusive: IntegrationStateRef,
        upper_bound_inclusive: IntegrationStateRef,
        transitions: Vec<ChangeUnit>,
        confidence: Confidence,
        reasons: Vec<IntegrationAmbiguityReason>,
    },
    NonMonotonic {
        lower_bound_exclusive: Option<IntegrationStateRef>,
        upper_bound_inclusive: Option<IntegrationStateRef>,
        transitions: Vec<ChangeUnit>,
        confidence: Option<Confidence>,
        reasons: Vec<IntegrationAmbiguityReason>,
    },
    Inconclusive {
        reasons: Vec<IntegrationAmbiguityReason>,
    },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum IntegrationError {
    #[error("integration provider must not be empty")]
    EmptyProvider,
    #[error("integration snapshot identity must not be empty")]
    EmptySnapshotId,
    #[error("integration sequence requires at least two states, found {actual}")]
    SequenceTooShort { actual: usize },
    #[error("integration state at ordinal {ordinal} has an empty state identity")]
    EmptyStateId { ordinal: usize },
    #[error("integration state at ordinal {ordinal} has an empty commit identity")]
    EmptyCommit { ordinal: usize },
    #[error("integration state at ordinal {ordinal} has an empty tree identity")]
    EmptyTree { ordinal: usize },
    #[error("integration state ordinal mismatch: expected {expected}, found {actual}")]
    OrdinalMismatch { expected: usize, actual: usize },
    #[error("duplicate integration state identity: {state_id}")]
    DuplicateStateId { state_id: String },
    #[error("duplicate integration commit identity: {commit}")]
    DuplicateCommit { commit: String },
    #[error("base integration state must not declare a transition")]
    BaseHasTransition,
    #[error("base integration state must not include queued changes")]
    BaseHasIncludedChanges,
    #[error("integration state at ordinal {ordinal} is missing its transition unit")]
    MissingTransition { ordinal: usize },
    #[error("integration state at ordinal {ordinal} repeats a prior change identity")]
    RepeatedTransition { ordinal: usize },
    #[error("integration state at ordinal {ordinal} contains a duplicate change unit")]
    DuplicateIncludedChange { ordinal: usize },
    #[error("integration state at ordinal {ordinal} is not the prior state plus one transition")]
    NonCumulativeState { ordinal: usize },
    #[error("invalid or cross-repository change identity at ordinal {ordinal}")]
    InvalidChange { ordinal: usize },
    #[error("state provenance does not match the captured sequence at ordinal {ordinal}")]
    ProvenanceMismatch { ordinal: usize },
    #[error("reconstructed state lacks a same-prefix equivalence witness at ordinal {ordinal}")]
    MissingEquivalenceWitness { ordinal: usize },
    #[error("localization boundary commit is not part of the integration sequence: {commit}")]
    UnknownBoundaryCommit { commit: String },
    #[error("invalid integration boundary order: {lower_ordinal} -> {upper_ordinal}")]
    InvalidBoundaryOrder {
        lower_ordinal: usize,
        upper_ordinal: usize,
    },
    #[error("integration state is missing at ordinal {ordinal}")]
    MissingStateAtOrdinal { ordinal: usize },
}
