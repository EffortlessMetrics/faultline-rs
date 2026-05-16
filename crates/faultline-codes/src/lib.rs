use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ObservationClass {
    Pass,
    Fail,
    Skip,
    Indeterminate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ProbeKind {
    Build,
    Test,
    Lint,
    PerfThreshold,
    Custom,
}

impl fmt::Display for ProbeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            ProbeKind::Build => "build",
            ProbeKind::Test => "test",
            ProbeKind::Lint => "lint",
            ProbeKind::PerfThreshold => "perf-threshold",
            ProbeKind::Custom => "custom",
        };
        write!(f, "{value}")
    }
}

impl FromStr for ProbeKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "build" => Ok(ProbeKind::Build),
            "test" => Ok(ProbeKind::Test),
            "lint" => Ok(ProbeKind::Lint),
            "perf" | "perf-threshold" | "perfgate" => Ok(ProbeKind::PerfThreshold),
            "custom" => Ok(ProbeKind::Custom),
            other => Err(format!("unsupported probe kind: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AmbiguityReason {
    MissingPassBoundary,
    MissingFailBoundary,
    NonMonotonicEvidence,
    SkippedRevision,
    IndeterminateRevision,
    UntestableWindow,
    BoundaryValidationFailed,
    NeedsMoreProbes,
    MaxProbesExhausted,
}

impl fmt::Display for AmbiguityReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            AmbiguityReason::MissingPassBoundary => "missing pass boundary",
            AmbiguityReason::MissingFailBoundary => "missing fail boundary",
            AmbiguityReason::NonMonotonicEvidence => "non-monotonic evidence",
            AmbiguityReason::SkippedRevision => "skipped revision",
            AmbiguityReason::IndeterminateRevision => "indeterminate revision",
            AmbiguityReason::UntestableWindow => "untestable window",
            AmbiguityReason::BoundaryValidationFailed => "boundary validation failed",
            AmbiguityReason::NeedsMoreProbes => "needs more probes",
            AmbiguityReason::MaxProbesExhausted => "max probes exhausted",
        };
        write!(f, "{text}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum OperatorCode {
    Success,
    SuspectWindow,
    Inconclusive,
    InvalidInput,
    ExecutionError,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Error, anyhow};

    /// Local fallible-eq helper: returns `Err` instead of panicking on mismatch.
    /// Mirrors `assert_eq!` so new tests stay out of the panic-family allowlist.
    fn check_eq<T: PartialEq + std::fmt::Debug>(
        left: T,
        right: T,
        context: &str,
    ) -> Result<(), Error> {
        if left != right {
            return Err(anyhow!("{context}: {left:?} != {right:?}"));
        }
        Ok(())
    }

    #[test]
    fn probe_kind_parses_build() -> Result<(), Error> {
        let kind = ProbeKind::from_str("build").map_err(|e| anyhow!("parse failed: {e}"))?;
        check_eq(kind, ProbeKind::Build, "build parses to Build")
    }

    #[test]
    fn probe_kind_parses_test() -> Result<(), Error> {
        let kind = ProbeKind::from_str("test").map_err(|e| anyhow!("parse failed: {e}"))?;
        check_eq(kind, ProbeKind::Test, "test parses to Test")
    }

    #[test]
    fn probe_kind_parses_lint() -> Result<(), Error> {
        let kind = ProbeKind::from_str("lint").map_err(|e| anyhow!("parse failed: {e}"))?;
        check_eq(kind, ProbeKind::Lint, "lint parses to Lint")
    }

    #[test]
    fn probe_kind_parses_perf_aliases() -> Result<(), Error> {
        for alias in ["perf", "perf-threshold", "perfgate"] {
            let kind = ProbeKind::from_str(alias)
                .map_err(|e| anyhow!("alias {alias} should parse: {e}"))?;
            check_eq(
                kind,
                ProbeKind::PerfThreshold,
                &format!("alias `{alias}` should map to PerfThreshold"),
            )?;
        }
        Ok(())
    }

    #[test]
    fn probe_kind_parses_custom() -> Result<(), Error> {
        let kind = ProbeKind::from_str("custom").map_err(|e| anyhow!("parse failed: {e}"))?;
        check_eq(kind, ProbeKind::Custom, "custom parses to Custom")
    }

    #[test]
    fn probe_kind_parser_is_case_insensitive() -> Result<(), Error> {
        let upper = ProbeKind::from_str("BUILD").map_err(|e| anyhow!("BUILD failed: {e}"))?;
        check_eq(upper, ProbeKind::Build, "uppercase BUILD parses to Build")?;

        let mixed = ProbeKind::from_str("Test").map_err(|e| anyhow!("Test failed: {e}"))?;
        check_eq(mixed, ProbeKind::Test, "mixed-case Test parses to Test")?;

        let with_ws =
            ProbeKind::from_str("  Lint  ").map_err(|e| anyhow!("padded Lint failed: {e}"))?;
        check_eq(
            with_ws,
            ProbeKind::Lint,
            "whitespace-padded Lint parses to Lint",
        )
    }

    #[test]
    fn probe_kind_parser_rejects_unknown_kind() -> Result<(), Error> {
        let err = match ProbeKind::from_str("definitely-not-a-real-kind") {
            Ok(k) => return Err(anyhow!("expected error for unknown kind, got Ok({k:?})")),
            Err(e) => e,
        };
        if !err.contains("unsupported probe kind") {
            return Err(anyhow!(
                "error message should mention 'unsupported probe kind', got: {err}"
            ));
        }
        if !err.contains("definitely-not-a-real-kind") {
            return Err(anyhow!(
                "error message should include the offending input, got: {err}"
            ));
        }
        Ok(())
    }

    #[test]
    fn probe_kind_display_round_trip() -> Result<(), Error> {
        let variants = [
            ProbeKind::Build,
            ProbeKind::Test,
            ProbeKind::Lint,
            ProbeKind::PerfThreshold,
            ProbeKind::Custom,
        ];
        for variant in variants {
            let rendered = variant.to_string();
            let parsed = ProbeKind::from_str(&rendered).map_err(|e| {
                anyhow!("display output `{rendered}` should round-trip via FromStr: {e}")
            })?;
            check_eq(
                parsed,
                variant,
                &format!("round-trip for {variant:?} via `{rendered}`"),
            )?;
        }
        Ok(())
    }

    #[test]
    fn ambiguity_reason_display_for_all_variants() -> Result<(), Error> {
        let cases = [
            (
                AmbiguityReason::MissingPassBoundary,
                "missing pass boundary",
            ),
            (
                AmbiguityReason::MissingFailBoundary,
                "missing fail boundary",
            ),
            (
                AmbiguityReason::NonMonotonicEvidence,
                "non-monotonic evidence",
            ),
            (AmbiguityReason::SkippedRevision, "skipped revision"),
            (
                AmbiguityReason::IndeterminateRevision,
                "indeterminate revision",
            ),
            (AmbiguityReason::UntestableWindow, "untestable window"),
            (
                AmbiguityReason::BoundaryValidationFailed,
                "boundary validation failed",
            ),
            (AmbiguityReason::NeedsMoreProbes, "needs more probes"),
            (AmbiguityReason::MaxProbesExhausted, "max probes exhausted"),
        ];
        for (variant, expected) in cases {
            let rendered = variant.to_string();
            if rendered != expected {
                return Err(anyhow!(
                    "AmbiguityReason::{variant:?} should display as `{expected}`, got `{rendered}`"
                ));
            }
        }
        Ok(())
    }
}
