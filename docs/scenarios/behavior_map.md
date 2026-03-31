# Behavior Map

Five-way cross-reference mapping requirements to ADRs, BDD scenarios, fixtures, and artifacts.

---

## Core Localization Requirements

| Requirement | ADR/Explanation | BDD Scenario | Fixture/Harness | Artifact/Output |
|-------------|----------------|--------------|-----------------|-----------------|
| 2.3 Exit code 0 → Pass | [ADR-0003](../adr/0003-honest-localization-outcomes.md), [Predicate Contract](../predicate-contract.md) | `prop_exit_code_classification` | `arb exit_code × timed_out` | — |
| 2.4 Exit code 125 → Skip | [ADR-0003](../adr/0003-honest-localization-outcomes.md), [Predicate Contract](../predicate-contract.md) | `prop_exit_code_classification` | `arb exit_code × timed_out` | — |
| 2.5 Non-zero exit → Fail | [ADR-0003](../adr/0003-honest-localization-outcomes.md), [Predicate Contract](../predicate-contract.md) | `prop_exit_code_classification` | `arb exit_code × timed_out` | — |
| 2.6 Timeout → Indeterminate | [ADR-0003](../adr/0003-honest-localization-outcomes.md), [Predicate Contract](../predicate-contract.md) | `prop_exit_code_classification` | `arb exit_code × timed_out` | — |
| 2.7 Observation structural completeness | [ADR-0003](../adr/0003-honest-localization-outcomes.md) | `prop_observation_structural_completeness` | `arb_commit_id() × arb_probe_kind()` | — |
| 3.1 Binary narrowing selects valid midpoint | [ADR-0001](../adr/0001-hexagonal-architecture.md) | `prop_binary_narrowing_selects_valid_midpoint` | `n in 3..=50` | — |
| 3.2 Adjacent Pass-Fail → FirstBad | [ADR-0003](../adr/0003-honest-localization-outcomes.md) | `prop_adjacent_pass_fail_yields_first_bad`, `exact_boundary_when_adjacent` | `n in 2..=20`, `make_seq` | — |
| 3.3 Skip/Indeterminate → SuspectWindow | [ADR-0003](../adr/0003-honest-localization-outcomes.md) | `prop_ambiguous_observations_yield_suspect_window`, `skipped_midpoint_yields_suspect_window` | `n in 4..=20`, `make_seq` | — |
| 3.4 Ambiguity reasons in SuspectWindow | [ADR-0003](../adr/0003-honest-localization-outcomes.md) | `prop_ambiguous_observations_yield_suspect_window`, `indeterminate_midpoint_yields_suspect_window` | `n in 4..=20`, `make_seq` | — |
| 3.5 Non-monotonic → low confidence | [ADR-0003](../adr/0003-honest-localization-outcomes.md) | `prop_non_monotonic_evidence_yields_low_confidence`, `non_monotonic_evidence_yields_low_confidence` | `n in 4..=20`, `make_seq` | — |
| 3.6 Missing Pass boundary → Inconclusive | [ADR-0003](../adr/0003-honest-localization-outcomes.md) | `prop_only_fail_yields_inconclusive_missing_pass_boundary`, `missing_pass_boundary_yields_inconclusive` | `n in 3..=15`, `make_seq` | — |
| 3.7 Missing Fail boundary → Inconclusive | [ADR-0003](../adr/0003-honest-localization-outcomes.md) | `prop_only_pass_yields_inconclusive_missing_fail_boundary`, `missing_fail_boundary_yields_inconclusive` | `n in 3..=15`, `make_seq` | — |
| 3.8 Probe count respects max_probes | [ADR-0001](../adr/0001-hexagonal-architecture.md) | `prop_probe_count_respects_max_probes` | `max_probes in 1..=10`, mock ports | — |
| 3.9 FirstBad requires direct evidence | [ADR-0003](../adr/0003-honest-localization-outcomes.md) | `prop_first_bad_requires_direct_evidence` | `n in 2..=20`, `i_frac` | — |

## Surface Analysis Requirements

| Requirement | ADR/Explanation | BDD Scenario | Fixture/Harness | Artifact/Output |
|-------------|----------------|--------------|-----------------|-----------------|
| 5.2 Path-based bucketing | [Architecture](../architecture.md) | `prop_surface_analysis_invariants`, `summarize_groups_by_top_level_directory` | `arb_path_change()`, fixed paths | — |
| 5.3 Top-level directory grouping | [Architecture](../architecture.md) | `prop_surface_analysis_invariants`, `summarize_groups_by_top_level_directory` | `arb_path_change()`, fixed paths | — |
| 5.4 Execution surface separation | [Architecture](../architecture.md) | `prop_surface_analysis_invariants`, `summarize_collects_execution_surfaces` | `arb_path_change()`, fixed paths | — |

## Artifact and Evidence Requirements

| Requirement | ADR/Explanation | BDD Scenario | Fixture/Harness | Artifact/Output |
|-------------|----------------|--------------|-----------------|-----------------|
| 6.1 analysis.json written | [Architecture](../architecture.md) | `render_writes_analysis_json` | Sample report + TempDir | `analysis.json` |
| 6.2 analysis.json field completeness | [Architecture](../architecture.md) | `analysis_json_contains_all_fields` | Sample report | `analysis.json` |
| 6.3 Deterministic JSON output | [Architecture](../architecture.md) | `prop_json_serialization_determinism`, `analysis_json_is_deterministic` | `arb_analysis_report()`, sample report | `analysis.json` |
| 6.5 JSON round-trip | [Architecture](../architecture.md) | `prop_analysis_report_json_round_trip`, `analysis_json_round_trips` | `arb_analysis_report()`, sample report | `analysis.json` |
| 7.1 index.html written | [Architecture](../architecture.md) | `render_writes_index_html`, `html_has_valid_structure` | Sample report + TempDir | `index.html` |
| 7.2 HTML content completeness | [Architecture](../architecture.md) | `prop_html_contains_required_data`, `html_contains_run_id` | `arb_analysis_report()`, sample report | `index.html` |
| 7.3 HTML self-contained | [Architecture](../architecture.md) | `prop_html_is_self_contained`, `html_has_no_external_dependencies` | `arb_analysis_report()`, sample report | `index.html` |
| 7.5 HTML escaping | [Architecture](../architecture.md) | `prop_html_escaping_correctness`, `escape_html_replaces_special_chars` | Arbitrary strings, XSS input | — |

## Repo Operating System Requirements

| Requirement | ADR/Explanation | BDD Scenario | Fixture/Harness | Artifact/Output |
|-------------|----------------|--------------|-----------------|-----------------|
| Req 1.1 Pattern catalog | [Pattern Catalog](../patterns/catalog.md) | `pattern_entry_structural_completeness` | `docs/patterns/catalog.md` | — |
| Req 1.2 Pattern entry structure | [Pattern Catalog](../patterns/catalog.md) | `pattern_entry_structural_completeness` | `docs/patterns/catalog.md` | — |
| Req 2.1 Scenario index | [Pattern: Scenario Atlas](../patterns/catalog.md#2-scenario-atlas) | `scenario_entry_structural_completeness` | `docs/scenarios/scenario_index.md` | — |
| Req 2.2 Scenario entry structure | [Pattern: Scenario Atlas](../patterns/catalog.md#2-scenario-atlas) | `scenario_entry_structural_completeness` | `docs/scenarios/scenario_index.md` | — |
| Req 2.5 Scenario atlas verification | [Pattern: Scenario Atlas](../patterns/catalog.md#2-scenario-atlas) | `prop_symmetric_difference_is_exact`, `check_consistency_reports_symmetric_difference` | `arb_test_name_set()`, hand-built sets | — |
| Req 3.1 JSON Schema generation | [Pattern: Artifact-First Boundary](../patterns/catalog.md#3-artifact-first-boundary) | `prop_json_schema_validates_all_valid_reports` | `arb_analysis_report()` | `schemas/analysis-report.schema.json` |
| Req 3.2 JSON Schema structure | [Pattern: Artifact-First Boundary](../patterns/catalog.md#3-artifact-first-boundary) | `prop_json_schema_validates_all_valid_reports` | `arb_analysis_report()` | `schemas/analysis-report.schema.json` |
| Req 3.4 Golden tests | [Pattern: Golden Artifact Contract](../patterns/catalog.md#9-golden-artifact-contract) | `golden_analysis_json`, `golden_index_html`, `golden_cli_help` | `canonical_fixture_report()`, `Cli::command()` | Insta snapshots |
| Req 3.6 SARIF export | [Pattern: Delegation-Safe Crate Seam](../patterns/catalog.md#10-delegation-safe-crate-seam) | `prop_sarif_export_structural_validity`, `sarif_first_bad_produces_error_level` | `arb_analysis_report()`, sample reports | SARIF JSON |
| Req 3.7 JUnit XML export | [Pattern: Delegation-Safe Crate Seam](../patterns/catalog.md#10-delegation-safe-crate-seam) | `prop_junit_xml_export_structural_validity`, `junit_first_bad_has_failure_element` | `arb_analysis_report()`, sample reports | JUnit XML |
| Req 4.1 AGENTS.md | [AGENTS.md](../../AGENTS.md) | — (teaching layer doc) | — | `AGENTS.md` |
| Req 4.2 TESTING.md | [TESTING.md](../../TESTING.md) | — (teaching layer doc) | — | `TESTING.md` |
| Req 4.5 Crate map | [Crate Map](../crate-map.md) | — (teaching layer doc) | — | `docs/crate-map.md` |
| Req 4.7 Cross-references | [AGENTS.md](../../AGENTS.md) | — (verified by doc review) | — | — |
| Req 5.2 Xtask subcommands | [Pattern: Proof-Carrying Change](../patterns/catalog.md#4-proof-carrying-change) | `xtask_help_lists_all_subcommands` | `Cli::command()` | — |
| Req 5.5 Xtask help message | [Pattern: Proof-Carrying Change](../patterns/catalog.md#4-proof-carrying-change) | `xtask_help_lists_all_subcommands` | `Cli::command()` | — |
| Req 5.7 Tool detection errors | [Pattern: Proof-Carrying Change](../patterns/catalog.md#4-proof-carrying-change) | `tool_detection_error_messages_known_tools`, `tool_detection_error_message_format` | Known tool list, `arb name × install_cmd` | — |
| Req 6.1 Verification matrix | [Verification Matrix](../verification-matrix.md) | — (documentation artifact) | — | `docs/verification-matrix.md` |
| Req 6.2 Technique assignment by tier | [Verification Matrix](../verification-matrix.md) | — (documentation artifact) | — | `docs/verification-matrix.md` |
| Req 8.1 Exit code mapping | [Pattern: Operator Dossier](../patterns/catalog.md#6-operator-dossier) | `prop_operator_code_exit_code_mapping` | `arb_localization_outcome()` | — |
| Req 8.3 Schema drift detection | [Pattern: Artifact-First Boundary](../patterns/catalog.md#3-artifact-first-boundary) | `prop_schema_drift_detection` | Generated schema + modified file | — |
| Req 8.4 Missing scenario detection | [Pattern: Scenario Atlas](../patterns/catalog.md#2-scenario-atlas) | `prop_symmetric_difference_is_exact`, `check_consistency_reports_symmetric_difference` | `arb_test_name_set()`, hand-built sets | — |
| Req 8.7 Contract-aware failure messages | [TESTING.md](../../TESTING.md) | `ci_failure_messages_identify_broken_contract`, `ci_contract_broken_message_contains_contract_name` | Known contracts, `arb contract name` | — |
| Req 8.8 Outcome visual distinction | [Pattern: Operator Dossier](../patterns/catalog.md#6-operator-dossier) | `prop_html_outcome_visual_distinction_and_badges` | `arb_analysis_report()` | `index.html` |
| Req 8.9 Ambiguity reason badges | [Pattern: Operator Dossier](../patterns/catalog.md#6-operator-dossier) | `prop_html_outcome_visual_distinction_and_badges` | `arb_analysis_report()` | `index.html` |
| Req 8.10 Temporal observation order | [Pattern: Operator Dossier](../patterns/catalog.md#6-operator-dossier) | `prop_html_temporal_observation_order` | `arb_analysis_report()` | `index.html` |
| Req 8.11 Execution surface separation | [Pattern: Operator Dossier](../patterns/catalog.md#6-operator-dossier) | `prop_html_execution_surface_separation` | `arb_analysis_report()` | `index.html` |
| Req 9.6 CLI help flag completeness | [Pattern: Operator Dossier](../patterns/catalog.md#6-operator-dossier) | `prop_cli_help_flag_completeness` | `any::<u32>()` | — |
| Req 10.1 Scaffold subcommands | [Pattern: Delegation-Safe Crate Seam](../patterns/catalog.md#10-delegation-safe-crate-seam) | `scaffold_help_lists_all_kinds` | `Cli::command()` | — |
| Req 10.2 Scaffold crate generation | [Pattern: Delegation-Safe Crate Seam](../patterns/catalog.md#10-delegation-safe-crate-seam) | `prop_scaffold_crate_generation` | `arb_crate_suffix() × arb_tier()` | Cargo.toml, lib.rs |
| Req 10.3 Scaffold ADR numbering | [Pattern: Delegation-Safe Crate Seam](../patterns/catalog.md#10-delegation-safe-crate-seam) | `prop_scaffold_adr_sequential_numbering` | `existing_count in 0..20` | ADR file |
| Req 10.4 Scaffold scenario generation | [Pattern: Delegation-Safe Crate Seam](../patterns/catalog.md#10-delegation-safe-crate-seam) | `prop_scaffold_scenario_creates_stub_and_index` | `arb scenario name` | test stub, index entry |
| Req 10.5 Scaffold doc generation | [Pattern: Delegation-Safe Crate Seam](../patterns/catalog.md#10-delegation-safe-crate-seam) | `prop_scaffold_doc_creates_file_and_summary_entry` | `arb section` | doc file, SUMMARY entry |
| Req 10.6 Scaffold input validation | [Pattern: Delegation-Safe Crate Seam](../patterns/catalog.md#10-delegation-safe-crate-seam) | `prop_scaffold_rejects_invalid_crate_names`, `prop_scaffold_rejects_invalid_doc_sections` | Invalid name patterns | — |
| Req 10.1–10.5 Boundary validation | [ADR-0003](../adr/0003-honest-localization-outcomes.md) | `prop_good_boundary_fail_yields_invalid_boundary`, `prop_bad_boundary_pass_yields_invalid_boundary` | `n in 3..=20`, mock ports | — |

## Hardening Requirements

| Requirement | ADR/Explanation | BDD Scenario | Fixture/Harness | Artifact/Output |
|-------------|----------------|--------------|-----------------|-----------------|
| Req 5.1 Signal-aware classification | [Predicate Contract](../predicate-contract.md) | `prop_signal_aware_exit_code_classification`, `signal_termination_sets_signal_number` | `arb exit_code × signal_number`, shell probe | — |
| Req 5.4 Probe diagnostic fields | [ADR-0003](../adr/0003-honest-localization-outcomes.md) | `prop_observation_structural_completeness` | `arb_commit_id() × arb_probe_kind()` | — |
| Req 11.1 FirstBad direct evidence | [ADR-0003](../adr/0003-honest-localization-outcomes.md) | `prop_first_bad_requires_direct_evidence` | `n in 2..=20`, `i_frac` | — |
| Req 11.2 Monotonic window narrowing | [ADR-0003](../adr/0003-honest-localization-outcomes.md) | `prop_monotonic_window_narrowing` | `n in 3..=20`, `transition_frac` | — |
| Req 11.3 SuspectWindow confidence cap | [ADR-0003](../adr/0003-honest-localization-outcomes.md) | `prop_suspect_window_confidence_cap` | `n in 4..=20`, `ambig_selectors` | — |
| Req 11.4 Observation order independence | [ADR-0003](../adr/0003-honest-localization-outcomes.md) | `prop_observation_order_independence` | `n in 3..=12`, `perm_seeds` | — |
