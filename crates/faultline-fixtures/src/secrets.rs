//! Secret pattern fixture corpus for property-based testing.
//!
//! Provides:
//! - `KNOWN_SECRETS` — strings that SHOULD be scrubbed by `scrub_secrets()`
//! - `FALSE_POSITIVES` — strings that should NOT be scrubbed
//! - `arb_analysis_report_with_sentinels()` — proptest strategy generating reports
//!   with UUID-prefixed sentinel env values for verifying no raw values leak

use crate::arb::*;
use faultline_types::*;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Known secrets — these SHOULD be detected and scrubbed
// ---------------------------------------------------------------------------

/// Strings that SHOULD be scrubbed by the secret scrubber.
/// Each entry matches one of the `SECRET_PATTERNS` regexes.
pub const KNOWN_SECRETS: &[&str] = &[
    // GitHub tokens (ghp_, gho_, ghu_, ghs_, ghr_ + 36+ alphanumeric chars)
    "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij",
    "gho_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij",
    "ghu_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij",
    "ghs_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij",
    "ghr_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij",
    // AWS access keys (AKIA + 16 uppercase alphanumeric)
    "AKIAIOSFODNN7EXAMPLE",
    "AKIA1234567890ABCDEF",
    // Stripe keys (sk-live_ or sk-test_ + 24+ alphanumeric)
    "sk-live_abcdefghijklmnopqrstuvwx",
    "sk-test_ABCDEFGHIJKLMNOPQRSTUVWX",
    // Bearer tokens (Bearer + non-whitespace)
    "Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0",
    "Bearer some-opaque-token-value",
    // password= followed by non-whitespace value
    // (values are obviously-synthetic placeholders to avoid tripping secret
    // scanners; the redaction regex only needs `password=` + non-whitespace)
    "password=hunter2",
    "password=FIXTURE_VALUE_NOT_REAL",
    // GitHub fine-grained PAT (github_pat_ + 40+ token body; underscore body is obviously synthetic)
    "github_pat__________________________________________________",
    // Google API key (AIza + 35 URL-safe chars; underscore body is obviously synthetic)
    "AIza___________________________________",
    // Slack bot token (xoxb- + 10+ chars; FIXTURE body is obviously synthetic)
    "xoxb-NOT-A-REAL-SLACK-TOKEN-FIXTURE",
    // PEM private key block (FIXTURE_ body is obviously not a real key)
    "-----BEGIN FIXTURE PRIVATE KEY-----\nFIXTURE_KEY_CONTENT_NOT_REAL\n-----END FIXTURE PRIVATE KEY-----",
];

// ---------------------------------------------------------------------------
// False positives — these should NOT be scrubbed
// ---------------------------------------------------------------------------

/// Strings that should NOT be scrubbed by the secret scrubber.
/// These are near-misses that look similar but don't match the patterns.
pub const FALSE_POSITIVES: &[&str] = &[
    // ghp_ prefix but too short (fewer than 36 alphanumeric chars after prefix)
    "ghp_short",
    "ghp_only15charshere",
    // AKIA alone with no 16-char suffix
    "AKIA",
    "AKIA_short",
    // Bearer alone with no token following
    "Bearer",
    "Bearer ",
    // password alone with no =value
    "password",
    "password_field",
    // sk-live_ with too short suffix (fewer than 24 chars)
    "sk-live_short",
    // Looks like a prefix but isn't quite right
    "ghx_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij",
    "BKIA1234567890ABCDEF",
    // github_pat_ prefix but too short (fewer than 40 chars after prefix)
    "github_pat_short",
    // AIza alone with no 35-char suffix
    "AIza",
    "AIza_short",
    // xox- prefix but not a recognized Slack type letter, or too short
    "xoxz-notaslacktoken",
    "xoxb-short",
    // Looks like a PEM header but no matching END (not a full block)
    "-----BEGIN RSA PRIVATE KEY-----",
    "-----BEGIN PRIVATE KEY-----\nnotakey\n-----END CERTIFICATE-----",
];

// ---------------------------------------------------------------------------
// Sentinel env value strategy
// ---------------------------------------------------------------------------

/// Prefix used for sentinel env values in property tests.
/// Easy to search for in output to verify no raw values leak.
pub const SENTINEL_PREFIX: &str = "SENTINEL_";

/// Generate a sentinel env value: `SENTINEL_` followed by a UUID-like hex string.
/// These are designed to be unique and easily searchable in output.
fn arb_sentinel_value() -> impl Strategy<Value = String> {
    "[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}"
        .prop_map(|uuid| format!("{}{}", SENTINEL_PREFIX, uuid))
}

/// Generate a valid shell identifier for use as an env key.
fn arb_env_key() -> impl Strategy<Value = String> {
    "[A-Z][A-Z0-9_]{2,10}".prop_map(|s| s)
}

/// Generate a non-empty vec of env pairs with sentinel values.
fn arb_sentinel_env_pairs() -> impl Strategy<Value = Vec<(String, String)>> {
    prop::collection::vec((arb_env_key(), arb_sentinel_value()), 1..5)
}

/// Generate a `ProbeSpec` with sentinel env values injected.
fn arb_probe_spec_with_sentinels() -> impl Strategy<Value = ProbeSpec> {
    prop_oneof![
        (
            arb_probe_kind(),
            "[a-z]{1,10}",
            prop::collection::vec("[a-z0-9]{1,8}", 0..3),
            arb_sentinel_env_pairs(),
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
            arb_sentinel_env_pairs(),
            1u64..600,
        )
            .prop_map(|(kind, shell, script, env, timeout_seconds)| {
                ProbeSpec::Shell {
                    kind,
                    shell,
                    script,
                    env,
                    timeout_seconds,
                }
            }),
    ]
}

/// Generate a `ReproductionCapsule` with sentinel env values.
fn arb_reproduction_capsule_with_sentinels() -> impl Strategy<Value = ReproductionCapsule> {
    (
        arb_commit_id(),
        arb_probe_spec_with_sentinels(),
        arb_sentinel_env_pairs(),
        "[a-z/]{1,20}",
        1u64..600,
    )
        .prop_map(|(commit, predicate, env, working_dir, timeout_seconds)| {
            ReproductionCapsule {
                commit,
                predicate,
                env,
                working_dir,
                timeout_seconds,
            }
        })
}

/// A proptest strategy that generates `AnalysisReport` instances with
/// UUID-prefixed sentinel env values injected into all env surfaces.
///
/// Use this in property tests to verify that no raw sentinel values
/// appear in redacted output. The sentinel prefix (`SENTINEL_`) makes
/// it trivial to search for leaked values.
///
/// Returns `(AnalysisReport, Vec<String>)` where the second element
/// is the list of all sentinel values injected, for easy assertion.
pub fn arb_analysis_report_with_sentinels() -> impl Strategy<Value = (AnalysisReport, Vec<String>)>
{
    // Split into two nested tuples to stay within proptest's 12-element limit
    let part1 = (
        "[a-z0-9-]{1,20}",
        any::<u64>(),
        arb_revision_spec(),
        arb_revision_spec(),
        arb_history_mode(),
        arb_probe_spec_with_sentinels(),
        arb_search_policy(),
    );
    let part2 = (
        arb_revision_sequence(),
        prop::collection::vec(arb_probe_observation(), 0..5),
        arb_localization_outcome(),
        prop::collection::vec(arb_path_change(), 0..5),
        arb_surface_summary(),
        prop::collection::vec(arb_reproduction_capsule_with_sentinels(), 1..3),
    );

    (part1, part2).prop_map(
        |(
            (run_id, created_at_epoch_seconds, good, bad, history_mode, probe, policy),
            (sequence, observations, outcome, changed_paths, surface, reproduction_capsules),
        )| {
            // Collect all sentinel values from all env surfaces
            let mut sentinels: Vec<String> = Vec::new();

            // From the request probe spec env
            match &probe {
                ProbeSpec::Exec { env, .. } | ProbeSpec::Shell { env, .. } => {
                    for (_, v) in env {
                        sentinels.push(v.clone());
                    }
                }
            }

            // From reproduction capsules
            for capsule in &reproduction_capsules {
                for (_, v) in &capsule.env {
                    sentinels.push(v.clone());
                }
                match &capsule.predicate {
                    ProbeSpec::Exec { env, .. } | ProbeSpec::Shell { env, .. } => {
                        for (_, v) in env {
                            sentinels.push(v.clone());
                        }
                    }
                }
            }

            let request = AnalysisRequest {
                repo_root: std::path::PathBuf::from("/tmp/repo"),
                good,
                bad,
                history_mode,
                probe,
                policy,
            };

            let report = AnalysisReport {
                schema_version: "0.3.0".into(),
                run_id,
                created_at_epoch_seconds,
                request,
                sequence,
                observations,
                outcome,
                changed_paths,
                surface,
                suspect_surface: vec![],
                reproduction_capsules,
                provenance: None,
            };

            (report, sentinels)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use faultline_types::scrub_secrets;

    #[test]
    fn known_secrets_are_scrubbed() {
        for secret in KNOWN_SECRETS {
            let scrubbed = scrub_secrets(secret);
            assert_ne!(
                &scrubbed, secret,
                "Expected '{}' to be scrubbed, but it was unchanged",
                secret
            );
            assert!(
                scrubbed.contains("[REDACTED]"),
                "Expected scrubbed output to contain [REDACTED], got: '{}'",
                scrubbed
            );
        }
    }

    #[test]
    fn false_positives_are_not_scrubbed() {
        for fp in FALSE_POSITIVES {
            let scrubbed = scrub_secrets(fp);
            assert_eq!(
                &scrubbed, fp,
                "Expected '{}' to NOT be scrubbed, but got: '{}'",
                fp, scrubbed
            );
        }
    }

    #[test]
    fn sentinel_values_have_correct_prefix() {
        use proptest::test_runner::{Config, TestRunner};

        let mut runner = TestRunner::new(Config {
            cases: 20,
            ..Config::default()
        });

        runner
            .run(&arb_sentinel_value(), |val| {
                prop_assert!(
                    val.starts_with(SENTINEL_PREFIX),
                    "Sentinel value '{}' should start with '{}'",
                    val,
                    SENTINEL_PREFIX
                );
                // UUID-like portion: 8-4-4-4-12 = 36 chars including hyphens
                let suffix = &val[SENTINEL_PREFIX.len()..];
                prop_assert_eq!(suffix.len(), 36, "UUID portion should be 36 chars");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn arb_report_with_sentinels_has_non_empty_sentinels() {
        use proptest::test_runner::{Config, TestRunner};

        let mut runner = TestRunner::new(Config {
            cases: 10,
            ..Config::default()
        });

        runner
            .run(
                &arb_analysis_report_with_sentinels(),
                |(report, sentinels)| {
                    prop_assert!(
                        !sentinels.is_empty(),
                        "Should have at least one sentinel value"
                    );
                    // All sentinels should start with the prefix
                    for s in &sentinels {
                        prop_assert!(
                            s.starts_with(SENTINEL_PREFIX),
                            "Sentinel '{}' missing prefix",
                            s
                        );
                    }
                    // Report should have reproduction capsules with env pairs
                    prop_assert!(
                        !report.reproduction_capsules.is_empty(),
                        "Report should have at least one reproduction capsule"
                    );
                    Ok(())
                },
            )
            .unwrap();
    }
}
