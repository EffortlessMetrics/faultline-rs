use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationClass {
    Pass,
    Fail,
    Skip,
    Indeterminate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
        write!(f, "{}", value)
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmbiguityReason {
    MissingPassBoundary,
    MissingFailBoundary,
    NonMonotonicEvidence,
    SkippedRevision,
    IndeterminateRevision,
    UntestableWindow,
    BoundaryValidationFailed,
    NeedsMoreProbes,
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
        };
        write!(f, "{}", text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatorCode {
    Success,
    SuspectWindow,
    Inconclusive,
    InvalidInput,
    ExecutionError,
}
