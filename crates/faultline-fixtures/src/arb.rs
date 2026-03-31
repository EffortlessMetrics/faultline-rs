//! Reusable proptest strategies for faultline domain types.
//!
//! These strategies mirror the ones in `faultline-types` tests but are
//! publicly exported so that adapter crates (`faultline-sarif`,
//! `faultline-junit`, etc.) can use them in their own property tests.

use faultline_codes::{AmbiguityReason, ObservationClass, ProbeKind};
use faultline_types::*;
use proptest::prelude::*;
use std::path::PathBuf;

pub fn arb_commit_id() -> impl Strategy<Value = CommitId> {
    "[a-f0-9]{8,40}".prop_map(CommitId)
}

pub fn arb_revision_spec() -> impl Strategy<Value = RevisionSpec> {
    "[a-f0-9]{8,40}".prop_map(RevisionSpec)
}

pub fn arb_history_mode() -> impl Strategy<Value = HistoryMode> {
    prop_oneof![
        Just(HistoryMode::AncestryPath),
        Just(HistoryMode::FirstParent),
    ]
}

pub fn arb_probe_kind() -> impl Strategy<Value = ProbeKind> {
    prop_oneof![
        Just(ProbeKind::Build),
        Just(ProbeKind::Test),
        Just(ProbeKind::Lint),
        Just(ProbeKind::PerfThreshold),
        Just(ProbeKind::Custom),
    ]
}

pub fn arb_shell_kind() -> impl Strategy<Value = ShellKind> {
    prop_oneof![
        Just(ShellKind::Default),
        Just(ShellKind::PosixSh),
        Just(ShellKind::Cmd),
        Just(ShellKind::PowerShell),
    ]
}

pub fn arb_probe_spec() -> impl Strategy<Value = ProbeSpec> {
    prop_oneof![
        (
            arb_probe_kind(),
            "[a-z]{1,10}",
            prop::collection::vec("[a-z0-9]{1,8}", 0..3),
            prop::collection::vec(("[A-Z]{1,4}", "[a-z0-9]{1,6}"), 0..2),
            1u64..600,
        )
            .prop_map(|(kind, program, args, env, timeout_seconds)| {
                ProbeSpec::Exec {
                    kind,
                    program,
                    args,
                    env,
                    timeout_seconds,
                }
            }),
        (
            arb_probe_kind(),
            arb_shell_kind(),
            "[a-z ]{1,20}",
            1u64..600,
        )
            .prop_map(|(kind, shell, script, timeout_seconds)| {
                ProbeSpec::Shell {
                    kind,
                    shell,
                    script,
                    env: vec![],
                    timeout_seconds,
                }
            }),
    ]
}

pub fn arb_search_policy() -> impl Strategy<Value = SearchPolicy> {
    (1usize..128).prop_map(|max_probes| SearchPolicy { max_probes })
}

pub fn arb_analysis_request() -> impl Strategy<Value = AnalysisRequest> {
    (
        "[a-z/]{1,20}",
        arb_revision_spec(),
        arb_revision_spec(),
        arb_history_mode(),
        arb_probe_spec(),
        arb_search_policy(),
    )
        .prop_map(
            |(repo_root, good, bad, history_mode, probe, policy)| AnalysisRequest {
                repo_root: PathBuf::from(repo_root),
                good,
                bad,
                history_mode,
                probe,
                policy,
            },
        )
}

pub fn arb_revision_sequence() -> impl Strategy<Value = RevisionSequence> {
    prop::collection::vec(arb_commit_id(), 2..10)
        .prop_map(|revisions| RevisionSequence { revisions })
}

pub fn arb_observation_class() -> impl Strategy<Value = ObservationClass> {
    prop_oneof![
        Just(ObservationClass::Pass),
        Just(ObservationClass::Fail),
        Just(ObservationClass::Skip),
        Just(ObservationClass::Indeterminate),
    ]
}

pub fn arb_probe_observation() -> impl Strategy<Value = ProbeObservation> {
    (
        arb_commit_id(),
        arb_observation_class(),
        arb_probe_kind(),
        prop::option::of(any::<i32>()),
        any::<bool>(),
        any::<u64>(),
        "[a-z ]{0,20}",
        "[a-z ]{0,20}",
        any::<u64>(),
        prop::option::of(any::<i32>()),
        "[a-z/ ]{0,30}",
        "[a-z/ ]{0,30}",
    )
        .prop_map(
            |(
                commit,
                class,
                kind,
                exit_code,
                timed_out,
                duration_ms,
                stdout,
                stderr,
                sequence_index,
                signal_number,
                probe_command,
                working_dir,
            )| {
                ProbeObservation {
                    commit,
                    class,
                    kind,
                    exit_code,
                    timed_out,
                    duration_ms,
                    stdout,
                    stderr,
                    sequence_index,
                    signal_number,
                    probe_command,
                    working_dir,
                }
            },
        )
}

pub fn arb_confidence() -> impl Strategy<Value = Confidence> {
    (any::<u8>(), "[a-z]{1,10}").prop_map(|(score, label)| Confidence { score, label })
}

pub fn arb_ambiguity_reason() -> impl Strategy<Value = AmbiguityReason> {
    prop_oneof![
        Just(AmbiguityReason::MissingPassBoundary),
        Just(AmbiguityReason::MissingFailBoundary),
        Just(AmbiguityReason::NonMonotonicEvidence),
        Just(AmbiguityReason::SkippedRevision),
        Just(AmbiguityReason::IndeterminateRevision),
        Just(AmbiguityReason::UntestableWindow),
        Just(AmbiguityReason::BoundaryValidationFailed),
        Just(AmbiguityReason::NeedsMoreProbes),
    ]
}

pub fn arb_localization_outcome() -> impl Strategy<Value = LocalizationOutcome> {
    prop_oneof![
        (arb_commit_id(), arb_commit_id(), arb_confidence()).prop_map(
            |(last_good, first_bad, confidence)| {
                LocalizationOutcome::FirstBad {
                    last_good,
                    first_bad,
                    confidence,
                }
            }
        ),
        (
            arb_commit_id(),
            arb_commit_id(),
            arb_confidence(),
            prop::collection::vec(arb_ambiguity_reason(), 1..4),
        )
            .prop_map(
                |(lower_bound_exclusive, upper_bound_inclusive, confidence, reasons)| {
                    LocalizationOutcome::SuspectWindow {
                        lower_bound_exclusive,
                        upper_bound_inclusive,
                        confidence,
                        reasons,
                    }
                }
            ),
        prop::collection::vec(arb_ambiguity_reason(), 1..4)
            .prop_map(|reasons| LocalizationOutcome::Inconclusive { reasons }),
    ]
}

pub fn arb_change_status() -> impl Strategy<Value = ChangeStatus> {
    prop_oneof![
        Just(ChangeStatus::Added),
        Just(ChangeStatus::Modified),
        Just(ChangeStatus::Deleted),
        Just(ChangeStatus::Renamed),
        Just(ChangeStatus::TypeChanged),
        Just(ChangeStatus::Unknown),
    ]
}

pub fn arb_path_change() -> impl Strategy<Value = PathChange> {
    (arb_change_status(), "[a-z/]{1,30}").prop_map(|(status, path)| PathChange { status, path })
}

pub fn arb_subsystem_bucket() -> impl Strategy<Value = SubsystemBucket> {
    (
        "[a-z]{1,10}",
        0usize..20,
        prop::collection::vec("[a-z/]{1,20}", 0..5),
        prop::collection::vec("[a-z]{1,10}", 0..3),
    )
        .prop_map(
            |(name, change_count, paths, surface_kinds)| SubsystemBucket {
                name,
                change_count,
                paths,
                surface_kinds,
            },
        )
}

pub fn arb_surface_summary() -> impl Strategy<Value = SurfaceSummary> {
    (
        0usize..50,
        prop::collection::vec(arb_subsystem_bucket(), 0..5),
        prop::collection::vec("[a-z/]{1,20}", 0..3),
    )
        .prop_map(
            |(total_changes, buckets, execution_surfaces)| SurfaceSummary {
                total_changes,
                buckets,
                execution_surfaces,
            },
        )
}

pub fn arb_analysis_report() -> impl Strategy<Value = AnalysisReport> {
    (
        "[a-z0-9-]{1,20}",
        any::<u64>(),
        arb_analysis_request(),
        arb_revision_sequence(),
        prop::collection::vec(arb_probe_observation(), 0..5),
        arb_localization_outcome(),
        prop::collection::vec(arb_path_change(), 0..5),
        arb_surface_summary(),
    )
        .prop_map(
            |(
                run_id,
                created_at_epoch_seconds,
                request,
                sequence,
                observations,
                outcome,
                changed_paths,
                surface,
            )| {
                AnalysisReport {
                    schema_version: "0.1.0".into(),
                    run_id,
                    created_at_epoch_seconds,
                    request,
                    sequence,
                    observations,
                    outcome,
                    changed_paths,
                    surface,
                }
            },
        )
}
