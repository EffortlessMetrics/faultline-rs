# Verification Matrix

This matrix maps each workspace crate to its applicable verification techniques. Techniques are assigned by crate tier following the hexagonal architecture.

## Verification Techniques by Crate

| Crate | Tier | Property | BDD/Unit | Golden | Fuzz | Mutation | Smoke |
|-------|------|----------|----------|--------|------|----------|-------|
| `faultline-codes` | domain | ✓ (via types) | ✓ | — | — | — | — |
| `faultline-types` | domain | ✓ | ✓ | — | ✓ | — | — |
| `faultline-localization` | domain | ✓ | ✓ | — | — | ✓ | — |
| `faultline-surface` | domain | ✓ | ✓ | — | — | ✓ | — |
| `faultline-ports` | ports | — | — | — | — | — | — |
| `faultline-app` | app | ✓ | ✓ | — | — | ✓ | — |
| `faultline-git` | adapter | ✓ | ✓ | — | ✓ | ✓ | — |
| `faultline-probe-exec` | adapter | ✓ | ✓ | — | — | ✓ | — |
| `faultline-store` | adapter | ✓ | ✓ | — | ✓ | ✓ | — |
| `faultline-render` | adapter | ✓ | ✓ | ✓ | ✓ | ✓ | — |
| `faultline-cli` | entry | ✓ | ✓ | ✓ | ✓ | — | ✓ |
| `faultline-fixtures` | testing | — | — | — | — | — | — |
| `faultline-sarif` | adapter | ✓ | ✓ | ✓ | ✓ | ✓ | — |
| `faultline-junit` | adapter | ✓ | ✓ | ✓ | ✓ | ✓ | — |
| `xtask` | tooling | — | ✓ | — | — | — | — |

## Technique Definitions

- **Property**: `proptest`-based tests that verify invariants across many randomly generated inputs. Minimum 100 cases per property.
- **BDD/Unit**: `#[test]` functions that verify specific examples, edge cases, and integration flows.
- **Golden**: `insta` snapshot tests that capture expected artifact output for regression detection.
- **Fuzz**: `cargo-fuzz` targets that exercise deserialization and parsing paths with arbitrary input.
- **Mutation**: `cargo-mutants` runs that verify the test suite detects meaningful code changes.
- **Smoke**: End-to-end tests that build the CLI binary and run it against a real Git repository.

## Property Test Configuration

- Minimum iteration count: **100 cases** per property
- Framework: `proptest` with `ProptestConfig { cases: 100, .. ProptestConfig::default() }`
- All domain property tests run against pure logic with no I/O
- Adapter property tests may use `tempfile` for filesystem isolation

## Mutation Testing Budget

| Target | Scope | Command |
|--------|-------|---------|
| `faultline-localization` | Core narrowing logic + outcome classification | `cargo mutants -p faultline-localization -- --lib` |
| `faultline-app` | Orchestration loop + boundary validation + flake retry + capsule generation | `cargo mutants -p faultline-app -- --lib` |
| `faultline-git` | CODEOWNERS parsing + blame frequency + diff output handling | `cargo mutants -p faultline-git -- --lib` |
| `faultline-probe-exec` | Predicate execution + timeout enforcement + exit code classification | `cargo mutants -p faultline-probe-exec -- --lib` |
| `faultline-store` | Atomic writes + lock files + observation persistence | `cargo mutants -p faultline-store -- --lib` |
| `faultline-render` | HTML/JSON/Markdown rendering + suspect surface display + HTML escaping | `cargo mutants -p faultline-render -- --lib` |
| `faultline-sarif` | SARIF v2.1.0 export + suspect surface locations | `cargo mutants -p faultline-sarif -- --lib` |
| `faultline-junit` | JUnit XML export + suspect surface in system-out | `cargo mutants -p faultline-junit -- --lib` |
| `faultline-surface` | Suspect surface ranking + scoring + owner hint mapping | `cargo mutants -p faultline-surface -- --lib` |

Mutation testing is run via `cargo xtask mutants` (supports `--crate <name>` for targeted runs) and is part of the `ci-extended` tier (manual trigger or release branches).

## Fuzz Testing Budget

| Target | Scope | Default Duration |
|--------|-------|-----------------|
| `fuzz_analysis_report` | `AnalysisReport` JSON deserialization | 60 seconds |
| `fuzz_git_diff_parse` | Git adapter diff output parsing with arbitrary byte strings | 60 seconds |
| `fuzz_store_json` | Store JSON deserialization with malformed `observations.json` | 60 seconds |
| `fuzz_html_escape` | Renderer HTML escaping with adversarial strings | 60 seconds |
| `fuzz_cli_args` | CLI argument parsing via clap with arbitrary string vectors | 60 seconds |
| `fuzz_sarif_export` | SARIF serialization with arbitrary `AnalysisReport` JSON | 60 seconds |
| `fuzz_junit_export` | JUnit serialization with arbitrary `AnalysisReport` JSON | 60 seconds |

Fuzz testing is run via `cargo xtask fuzz --duration <seconds>` and is part of the `ci-extended` tier.

## CI Tiers

| Tier | Trigger | Techniques | Target Time |
|------|---------|------------|-------------|
| `ci-fast` | Every push | fmt + clippy + test (all crates) | < 5 minutes |
| `ci-full` | Pull requests | ci-fast + golden + schema-check | < 10 minutes |
| `ci-extended` | Manual / release | ci-full + mutation + fuzz + release-check | Variable |
