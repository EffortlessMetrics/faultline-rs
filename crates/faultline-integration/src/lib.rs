//! Pure domain model for ordered integration sequences.
//!
//! Faultline's localization engine searches an ordered sequence of immutable
//! candidate commits. This crate retains the integration meaning around that
//! sequence: which cumulative changes each state contains, which change unit
//! was introduced at each transition, and how strongly a reconstructed state
//! matches the provider state it represents.
//!
//! The crate performs no Git, filesystem, process, network, or provider I/O.
//!
//! # Example
//!
//! ```
//! use faultline_codes::AmbiguityReason;
//! use faultline_integration::{
//!     ChangeUnit, IntegrationOutcome, IntegrationSequence, IntegrationSnapshotId,
//!     IntegrationState, IntegrationStateId, StateEquivalence, StateProvenance, TreeId,
//! };
//! use faultline_types::{CommitId, Confidence, LocalizationOutcome};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let base = CommitId("base".into());
//! let head = CommitId("head".into());
//! let pull_request = ChangeUnit::PullRequest {
//!     provider: "github".into(),
//!     repository: "example/repo".into(),
//!     number: 14,
//!     head: CommitId("pr-head".into()),
//! };
//!
//! let sequence = IntegrationSequence::try_new(
//!     "github",
//!     IntegrationSnapshotId("incident-1".into()),
//!     vec![
//!         IntegrationState {
//!             id: IntegrationStateId("S0".into()),
//!             ordinal: 0,
//!             commit: base.clone(),
//!             tree: TreeId("tree-0".into()),
//!             included_changes: vec![],
//!             transition: None,
//!             equivalence: StateEquivalence::ExactCommit,
//!             provenance: StateProvenance::Provider {
//!                 provider: "github".into(),
//!                 snapshot: IntegrationSnapshotId("incident-1".into()),
//!             },
//!         },
//!         IntegrationState {
//!             id: IntegrationStateId("S1".into()),
//!             ordinal: 1,
//!             commit: head.clone(),
//!             tree: TreeId("tree-1".into()),
//!             included_changes: vec![pull_request.clone()],
//!             transition: Some(pull_request.clone()),
//!             equivalence: StateEquivalence::ExactCommit,
//!             provenance: StateProvenance::Provider {
//!                 provider: "github".into(),
//!                 snapshot: IntegrationSnapshotId("incident-1".into()),
//!             },
//!         },
//!     ],
//! )?;
//!
//! let result = sequence.map_localization_outcome(
//!     &LocalizationOutcome::FirstBad {
//!         last_good: base,
//!         first_bad: head,
//!         confidence: Confidence::high(),
//!     },
//!     StateEquivalence::ExactTree,
//! )?;
//!
//! assert!(matches!(
//!     result,
//!     IntegrationOutcome::FirstFailingTransition { transition, .. }
//!         if transition == pull_request
//! ));
//! let _ = AmbiguityReason::NeedsMoreProbes;
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]

use faultline_codes::AmbiguityReason;
use faultline_types::{CommitId, Confidence, LocalizationOutcome, RevisionSequence};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// Result type for integration-sequence validation and outcome mapping.
pub type Result<T> = std::result::Result<T, IntegrationError>;

/// Stable provider-neutral identity for one captured integration snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationSnapshotId(pub String);

/// Stable identity for one candidate state inside an integration sequence.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationStateId(pub String);

/// Git tree identity associated with an integration state.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct TreeId(pub String);

/// One provider-neutral unit introduced between adjacent integration states.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum ChangeUnit {
    /// A pull request at one immutable head commit.
    PullRequest {
        provider: String,
        repository: String,
        number: u64,
        head: CommitId,
    },
    /// A raw commit transition for providers without pull-request identity.
    Commit {
        repository: String,
        commit: CommitId,
    },
}

/// Proven relationship between a candidate and the provider state it models.
///
/// Declaration order is weakest to strongest so ordinary ordering operations
/// can compare whether an actual state satisfies a required minimum.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum StateEquivalence {
    /// No provider equivalence has been established.
    Unverified,
    /// A bounded patch-equivalence policy passed, but tree identity differs.
    PatchEquivalent,
    /// Repository bytes match exactly while commit metadata or history differs.
    ExactTree,
    /// Commit and tree identity both match the provider state.
    ExactCommit,
}

impl StateEquivalence {
    /// Return whether this actual equivalence satisfies a predicate's minimum.
    #[must_use]
    pub fn satisfies(self, required: Self) -> bool {
        self >= required
    }
}

/// Source of one integration state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum StateProvenance {
    /// Exact state retained from an integration provider.
    Provider {
        provider: String,
        snapshot: IntegrationSnapshotId,
    },
    /// State reconstructed from retained immutable inputs.
    Reconstructed {
        builder: String,
        source_snapshot: IntegrationSnapshotId,
    },
}

/// One cumulative candidate in an ordered integration experiment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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

/// Compact state identity embedded in an integration-facing outcome.
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

/// Ordered cumulative integration states supplied by a provider adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationSequence {
    pub provider: String,
    pub snapshot_id: IntegrationSnapshotId,
    pub states: Vec<IntegrationState>,
}

impl IntegrationSequence {
    /// Construct and validate an integration sequence.
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

    /// Validate ordering, identity and cumulative-membership invariants.
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

        let mut seen_state_ids = HashSet::new();
        let mut seen_commits = HashSet::new();
        let mut previous: Option<&IntegrationState> = None;

        for (expected_ordinal, state) in self.states.iter().enumerate() {
            if state.id.0.trim().is_empty() {
                return Err(IntegrationError::EmptyStateId {
                    ordinal: expected_ordinal,
                });
            }
            if state.commit.0.trim().is_empty() {
                return Err(IntegrationError::EmptyCommit {
                    ordinal: expected_ordinal,
                });
            }
            if state.tree.0.trim().is_empty() {
                return Err(IntegrationError::EmptyTree {
                    ordinal: expected_ordinal,
                });
            }
            if state.ordinal != expected_ordinal {
                return Err(IntegrationError::OrdinalMismatch {
                    expected: expected_ordinal,
                    actual: state.ordinal,
                });
            }
            if !seen_state_ids.insert(state.id.clone()) {
                return Err(IntegrationError::DuplicateStateId {
                    state_id: state.id.0.clone(),
                });
            }
            if !seen_commits.insert(state.commit.clone()) {
                return Err(IntegrationError::DuplicateCommit {
                    commit: state.commit.0.clone(),
                });
            }

            Self::validate_unique_changes(state)?;

            match previous {
                None => {
                    if state.transition.is_some() {
                        return Err(IntegrationError::BaseHasTransition);
                    }
                    if !state.included_changes.is_empty() {
                        return Err(IntegrationError::BaseHasIncludedChanges);
                    }
                }
                Some(previous_state) => {
                    let transition =
                        state
                            .transition
                            .as_ref()
                            .ok_or(IntegrationError::MissingTransition {
                                ordinal: state.ordinal,
                            })?;
                    if previous_state.included_changes.contains(transition) {
                        return Err(IntegrationError::RepeatedTransition {
                            ordinal: state.ordinal,
                        });
                    }

                    let mut expected_changes = previous_state.included_changes.clone();
                    expected_changes.push(transition.clone());
                    if state.included_changes != expected_changes {
                        return Err(IntegrationError::NonCumulativeState {
                            ordinal: state.ordinal,
                        });
                    }
                }
            }

            previous = Some(state);
        }

        Ok(())
    }

    /// Number of cumulative states, including the base state.
    #[must_use]
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// Whether this sequence contains no states.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Convert the validated integration sequence into localization candidates.
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

    /// Map a provider-neutral localization result back to integration states and
    /// transition units.
    pub fn map_localization_outcome(
        &self,
        outcome: &LocalizationOutcome,
        required_equivalence: StateEquivalence,
    ) -> Result<IntegrationOutcome> {
        self.validate()?;

        match outcome {
            LocalizationOutcome::FirstBad {
                last_good,
                first_bad,
                confidence,
            } => self.map_first_bad(last_good, first_bad, confidence, required_equivalence),
            LocalizationOutcome::SuspectWindow {
                lower_bound_exclusive,
                upper_bound_inclusive,
                confidence,
                reasons,
            } => self.map_suspect_window(
                lower_bound_exclusive,
                upper_bound_inclusive,
                confidence,
                reasons,
                required_equivalence,
            ),
            LocalizationOutcome::Inconclusive { reasons } => {
                let mapped_reasons = reasons
                    .iter()
                    .cloned()
                    .map(IntegrationAmbiguityReason::Localization)
                    .collect::<Vec<_>>();
                if Self::contains_non_monotonic(reasons) {
                    Ok(IntegrationOutcome::NonMonotonic {
                        lower_bound_exclusive: None,
                        upper_bound_inclusive: None,
                        transitions: Vec::new(),
                        confidence: None,
                        reasons: mapped_reasons,
                    })
                } else {
                    Ok(IntegrationOutcome::Inconclusive {
                        reasons: mapped_reasons,
                    })
                }
            }
        }
    }

    fn validate_unique_changes(state: &IntegrationState) -> Result<()> {
        let mut seen = HashSet::new();
        for change in &state.included_changes {
            if !seen.insert(change) {
                return Err(IntegrationError::DuplicateIncludedChange {
                    ordinal: state.ordinal,
                });
            }
        }
        Ok(())
    }

    fn map_first_bad(
        &self,
        last_good: &CommitId,
        first_bad: &CommitId,
        confidence: &Confidence,
        required_equivalence: StateEquivalence,
    ) -> Result<IntegrationOutcome> {
        let (lower_index, upper_index, lower_state, upper_state) =
            self.resolve_window(last_good, first_bad)?;
        let transitions = self.transitions_between(lower_index, upper_index)?;
        let boundary_equivalence = lower_state.equivalence.min(upper_state.equivalence);

        let mut reasons = self.equivalence_reasons(lower_index, upper_index, required_equivalence);
        if upper_index != lower_index + 1 {
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
                boundary_equivalence,
                confidence: confidence.clone(),
            });
        }

        Ok(IntegrationOutcome::SuspectTransitions {
            lower_bound_exclusive: lower_state.into(),
            upper_bound_inclusive: upper_state.into(),
            transitions,
            confidence: confidence.clone(),
            reasons,
        })
    }

    fn map_suspect_window(
        &self,
        lower: &CommitId,
        upper: &CommitId,
        confidence: &Confidence,
        core_reasons: &[AmbiguityReason],
        required_equivalence: StateEquivalence,
    ) -> Result<IntegrationOutcome> {
        let (lower_index, upper_index, lower_state, upper_state) =
            self.resolve_window(lower, upper)?;
        let transitions = self.transitions_between(lower_index, upper_index)?;
        let mut reasons = core_reasons
            .iter()
            .cloned()
            .map(IntegrationAmbiguityReason::Localization)
            .collect::<Vec<_>>();
        reasons.extend(self.equivalence_reasons(lower_index, upper_index, required_equivalence));

        if Self::contains_non_monotonic(core_reasons) {
            return Ok(IntegrationOutcome::NonMonotonic {
                lower_bound_exclusive: Some(lower_state.into()),
                upper_bound_inclusive: Some(upper_state.into()),
                transitions,
                confidence: Some(confidence.clone()),
                reasons,
            });
        }

        Ok(IntegrationOutcome::SuspectTransitions {
            lower_bound_exclusive: lower_state.into(),
            upper_bound_inclusive: upper_state.into(),
            transitions,
            confidence: confidence.clone(),
            reasons,
        })
    }

    fn resolve_window<'a>(
        &'a self,
        lower: &CommitId,
        upper: &CommitId,
    ) -> Result<(usize, usize, &'a IntegrationState, &'a IntegrationState)> {
        let lower_index =
            self.index_of_commit(lower)
                .ok_or_else(|| IntegrationError::UnknownBoundaryCommit {
                    commit: lower.0.clone(),
                })?;
        let upper_index =
            self.index_of_commit(upper)
                .ok_or_else(|| IntegrationError::UnknownBoundaryCommit {
                    commit: upper.0.clone(),
                })?;
        if upper_index <= lower_index {
            return Err(IntegrationError::InvalidBoundaryOrder {
                lower_ordinal: lower_index,
                upper_ordinal: upper_index,
            });
        }

        let lower_state =
            self.states
                .get(lower_index)
                .ok_or(IntegrationError::MissingStateAtOrdinal {
                    ordinal: lower_index,
                })?;
        let upper_state =
            self.states
                .get(upper_index)
                .ok_or(IntegrationError::MissingStateAtOrdinal {
                    ordinal: upper_index,
                })?;
        Ok((lower_index, upper_index, lower_state, upper_state))
    }

    fn index_of_commit(&self, commit: &CommitId) -> Option<usize> {
        self.states.iter().position(|state| state.commit == *commit)
    }

    fn transitions_between(
        &self,
        lower_index: usize,
        upper_index: usize,
    ) -> Result<Vec<ChangeUnit>> {
        let width = upper_index.saturating_sub(lower_index);
        let transitions = self
            .states
            .iter()
            .skip(lower_index.saturating_add(1))
            .take(width)
            .map(|state| {
                state
                    .transition
                    .clone()
                    .ok_or(IntegrationError::MissingTransition {
                        ordinal: state.ordinal,
                    })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(transitions)
    }

    fn equivalence_reasons(
        &self,
        lower_index: usize,
        upper_index: usize,
        required: StateEquivalence,
    ) -> Vec<IntegrationAmbiguityReason> {
        let width = upper_index.saturating_sub(lower_index).saturating_add(1);
        self.states
            .iter()
            .skip(lower_index)
            .take(width)
            .filter(|state| !state.equivalence.satisfies(required))
            .map(
                |state| IntegrationAmbiguityReason::InsufficientStateEquivalence {
                    ordinal: state.ordinal,
                    actual: state.equivalence,
                    required,
                },
            )
            .collect()
    }

    fn contains_non_monotonic(reasons: &[AmbiguityReason]) -> bool {
        reasons
            .iter()
            .any(|reason| matches!(reason, AmbiguityReason::NonMonotonicEvidence))
    }
}

/// Integration-specific reasons that prevent an exact transition claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum IntegrationAmbiguityReason {
    /// Ambiguity reported by the provider-neutral localization engine.
    Localization(AmbiguityReason),
    /// One or more states are weaker than the predicate requires.
    InsufficientStateEquivalence {
        ordinal: usize,
        actual: StateEquivalence,
        required: StateEquivalence,
    },
    /// A nominal first-bad result did not identify adjacent candidate states.
    NonAdjacentBoundary {
        lower_ordinal: usize,
        upper_ordinal: usize,
    },
}

/// Queue-facing localization result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum IntegrationOutcome {
    /// Stable adjacent pass/fail states identify one transition unit.
    FirstFailingTransition {
        last_passing: IntegrationStateRef,
        first_failing: IntegrationStateRef,
        transition: ChangeUnit,
        boundary_equivalence: StateEquivalence,
        confidence: Confidence,
    },
    /// The supported evidence narrows to an ordered set of transition units.
    SuspectTransitions {
        lower_bound_exclusive: IntegrationStateRef,
        upper_bound_inclusive: IntegrationStateRef,
        transitions: Vec<ChangeUnit>,
        confidence: Confidence,
        reasons: Vec<IntegrationAmbiguityReason>,
    },
    /// Observed evidence violates the one-way pass-to-fail model.
    NonMonotonic {
        lower_bound_exclusive: Option<IntegrationStateRef>,
        upper_bound_inclusive: Option<IntegrationStateRef>,
        transitions: Vec<ChangeUnit>,
        confidence: Option<Confidence>,
        reasons: Vec<IntegrationAmbiguityReason>,
    },
    /// Missing or ineligible evidence prevents a supported transition window.
    Inconclusive {
        reasons: Vec<IntegrationAmbiguityReason>,
    },
}

/// Integration-sequence validation or mapping failure.
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
    #[error("integration state at ordinal {ordinal} repeats a prior transition")]
    RepeatedTransition { ordinal: usize },
    #[error("integration state at ordinal {ordinal} contains a duplicate change unit")]
    DuplicateIncludedChange { ordinal: usize },
    #[error("integration state at ordinal {ordinal} is not the prior state plus one transition")]
    NonCumulativeState { ordinal: usize },
    #[error("localization boundary commit is not part of the integration sequence: {commit}")]
    UnknownBoundaryCommit { commit: String },
    #[error(
        "integration boundary order is invalid: lower ordinal {lower_ordinal}, upper ordinal {upper_ordinal}"
    )]
    InvalidBoundaryOrder {
        lower_ordinal: usize,
        upper_ordinal: usize,
    },
    #[error("integration state is missing at ordinal {ordinal}")]
    MissingStateAtOrdinal { ordinal: usize },
}
