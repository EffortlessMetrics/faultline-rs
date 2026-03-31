# Artifact Schema

Faultline produces a structured JSON report (`analysis.json`) for every run. The report conforms to a versioned JSON Schema.

## Schema location

The canonical schema is at [`schemas/analysis-report.schema.json`](../../../../schemas/analysis-report.schema.json) in the repository root.

The schema is auto-generated from the Rust types using `schemars`. To regenerate after type changes:

```bash
cargo xtask generate-schema
```

## Schema version

The `schema_version` field in `AnalysisReport` tracks the contract version. The current version is `0.1.0`. Breaking changes to the report structure require a version bump.

## Top-level structure

The `AnalysisReport` object contains:

| Field | Type | Description |
|-------|------|-------------|
| `run_id` | string | Unique identifier for this analysis run |
| `schema_version` | string | Schema contract version (default `"0.1.0"`) |
| `created_at_epoch_seconds` | integer | Unix timestamp of report creation |
| `request` | `AnalysisRequest` | The original analysis parameters |
| `sequence` | `RevisionSequence` | The commit sequence that was searched |
| `observations` | array of `ProbeObservation` | All probe results, ordered by `sequence_index` |
| `outcome` | `LocalizationOutcome` | The localization result (FirstBad, SuspectWindow, or Inconclusive) |
| `changed_paths` | array of `PathChange` | Files changed in the regression window |
| `surface` | `SurfaceSummary` | Coarse subsystem bucketing of changed paths |

## Key types

### LocalizationOutcome

A tagged union with three variants:

- **FirstBad** — `last_good`, `first_bad` (CommitId), `confidence`
- **SuspectWindow** — `lower_bound_exclusive`, `upper_bound_inclusive` (CommitId), `confidence`, `reasons` (array of AmbiguityReason)
- **Inconclusive** — `reasons` (array of AmbiguityReason)

### ProbeObservation

| Field | Type | Description |
|-------|------|-------------|
| `commit` | string | The commit hash that was probed |
| `class` | string | Observation class: `Pass`, `Fail`, `Skip`, `Indeterminate` |
| `kind` | string | Probe kind: `Build`, `Test`, `Lint`, `PerfThreshold`, `Custom` |
| `exit_code` | integer or null | Process exit code |
| `timed_out` | boolean | Whether the probe timed out |
| `duration_ms` | integer | Probe execution time in milliseconds |
| `sequence_index` | integer | Order in which this probe was executed |

### Confidence

| Field | Type | Description |
|-------|------|-------------|
| `score` | integer (0–100) | Numeric confidence score |
| `label` | string | Human-readable label: `high`, `medium`, `low` |

## Validation

To validate a report against the schema programmatically, use any JSON Schema draft-07 validator with the schema file.

## Export formats

Faultline also supports exporting reports to:

- **SARIF v2.1.0** — via `faultline-sarif` crate (`to_sarif(&report)`)
- **JUnit XML** — via `faultline-junit` crate (`to_junit_xml(&report)`)
