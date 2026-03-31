# Scenario Index

This index catalogs every BDD scenario and property test in the faultline workspace. Each entry includes the scenario name, problem description, fixture or generator, crate(s) exercised, artifact(s) produced or validated, invariant or property asserted, and related references.

Scenarios are organized by crate tier following the verification matrix.

---

## Domain Tier — Property Tests

### faultline-codes

_No dedicated tests. Enum types are exercised transitively through `faultline-types` and `faultline-localization` tests._

### faultline-types

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs |
|----------|---------|-------------------|----------|----------|-----------|------|
| `all_types_derive_required_traits` | All public types implement Serialize, Deserialize, Debug, Clone, PartialEq, Eq | Hand-built samples | faultline-types | — | Trait completeness | — |
| `stable_hash_deterministic` | FNV hash produces identical output for identical input | Fixed input `"hello world"` | faultline-types | — | Determinism | — |
| `stable_hash_different_inputs_differ` | Different inputs produce different hashes | Fixed inputs `"hello"`, `"world"` | faultline-types | — | Collision resistance | — |
| `stable_hash_returns_16_hex_chars` | Hash output is exactly 16 hex characters | Fixed input `"test"` | faultline-types | — | Format correctness | — |
| `now_epoch_seconds_returns_reasonable_value` | Timestamp is within a plausible range | System clock | faultline-types | — | Sanity check | — |
| `probe_spec_fingerprint_deterministic` | ProbeSpec fingerprint is deterministic | Sample ProbeSpec | faultline-types | — | Determinism | — |
| `probe_spec_fingerprint_differs_for_different_specs` | Different ProbeSpecs produce different fingerprints | Two distinct ProbeSpecs | faultline-types | — | Uniqueness | — |
| `analysis_request_fingerprint_deterministic` | AnalysisRequest fingerprint is deterministic | Sample AnalysisRequest | faultline-types | — | Determinism | — |
| `analysis_request_fingerprint_differs_for_different_requests` | Different requests produce different fingerprints | Two distinct AnalysisRequests | faultline-types | — | Uniqueness | — |
| `boundary_pair_first_bad` | FirstBad outcome returns correct boundary pair | Hand-built outcome | faultline-types | — | Accessor correctness | — |
| `boundary_pair_suspect_window` | SuspectWindow outcome returns correct boundary pair | Hand-built outcome | faultline-types | — | Accessor correctness | — |
| `boundary_pair_inconclusive_returns_none` | Inconclusive outcome returns None for boundary pair | Hand-built outcome | faultline-types | — | Accessor correctness | — |
| `confidence_high` | Confidence::high() returns score=95, label="high" | Constructor | faultline-types | — | Value correctness | — |
| `confidence_medium` | Confidence::medium() returns score=65, label="medium" | Constructor | faultline-types | — | Value correctness | — |
| `confidence_low` | Confidence::low() returns score=25, label="low" | Constructor | faultline-types | — | Value correctness | — |
| `search_policy_default` | Default SearchPolicy has max_probes=64 | Default constructor | faultline-types | — | Default correctness | — |
| `commit_id_display` | CommitId Display impl formats correctly | Fixed CommitId | faultline-types | — | Display correctness | — |
| `revision_sequence_len_and_is_empty` | RevisionSequence len/is_empty work correctly | Empty and non-empty sequences | faultline-types | — | Accessor correctness | — |
| `faultline_error_from_io` | FaultlineError::from(io::Error) produces Io variant | io::Error | faultline-types | — | Error conversion | — |
| `faultline_error_from_serde_json` | FaultlineError::from(serde_json::Error) produces Serde variant | Invalid JSON | faultline-types | — | Error conversion | — |
| `faultline_error_display` | All FaultlineError variants display correctly | Each variant | faultline-types | — | Display correctness | — |
| `prop_json_serialization_determinism` | Serializing the same AnalysisReport twice produces byte-identical JSON | `arb_analysis_report()` | faultline-types | — | P14: Determinism | Req 6.3 |
| `prop_analysis_report_json_round_trip` | JSON round-trip preserves equality for AnalysisReport | `arb_analysis_report()` | faultline-types | — | P15: Round-trip | Req 6.5 |
| `prop_json_schema_validates_all_valid_reports` | Generated JSON Schema accepts all valid AnalysisReport instances | `arb_analysis_report()` | faultline-types | — | P40: Schema validity | Req 3.1, 3.2 |

### faultline-localization

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs |
|----------|---------|-------------------|----------|----------|-----------|------|
| `new_rejects_empty_sequence` | Empty revision sequence is rejected | Empty RevisionSequence | faultline-localization | — | Input validation | — |
| `new_builds_index_by_commit` | Session builds correct commit→index mapping | `make_seq(["a","b","c"])` | faultline-localization | — | Index correctness | — |
| `record_rejects_unknown_commit` | Recording observation for unknown commit fails | `make_seq(["a","b"])` + unknown "z" | faultline-localization | — | Input validation | — |
| `record_accepts_known_commit` | Recording observation for known commit succeeds | `make_seq(["a","b"])` | faultline-localization | — | Happy path | — |
| `has_observation_and_get_observation` | Observation accessors work before and after recording | `make_seq(["a","b","c"])` | faultline-localization | — | Accessor correctness | — |
| `observation_list_returns_all_in_sequence_index_order` | observation_list() sorts by sequence_index | `make_seq` + obs_with_seq | faultline-localization | — | Sort order | — |
| `observation_list_orders_by_sequence_index_not_revision_position` | Sort is by sequence_index, not revision position | 5-commit sequence, binary-search order | faultline-localization | — | Sort order | — |
| `record_preserves_preassigned_sequence_index` | Pre-assigned sequence_index is preserved | `obs_with_seq("a", Pass, 42)` | faultline-localization | — | Value preservation | — |
| `sequence_accessor` | sequence() returns the original RevisionSequence | `make_seq(["a","b"])` | faultline-localization | — | Accessor correctness | — |
| `max_probes_accessor` | max_probes() returns the policy value | Policy with max_probes=42 | faultline-localization | — | Accessor correctness | — |
| `next_probe_probes_first_boundary_first` | First probe targets the first commit | `make_seq(["a","b","c"])` | faultline-localization | — | Boundary-first probing | — |
| `next_probe_probes_last_boundary_second` | Second probe targets the last commit | `make_seq` + Pass at first | faultline-localization | — | Boundary-first probing | — |
| `next_probe_binary_narrows_between_boundaries` | After boundaries, probe targets median unobserved | 5-commit seq, Pass/Fail at ends | faultline-localization | — | Binary narrowing | — |
| `next_probe_returns_none_when_converged` | No more probes when adjacent pass-fail found | 2-commit seq, Pass+Fail | faultline-localization | — | Convergence detection | — |
| `next_probe_respects_max_probes` | Stops probing when max_probes reached | max_probes=2, 5-commit seq | faultline-localization | — | Budget enforcement | — |
| `next_probe_single_element_returns_none` | Single-element sequence returns None | 1-commit seq | faultline-localization | — | Edge case | — |
| `exact_boundary_when_adjacent` | Adjacent Pass-Fail yields FirstBad | 3-commit seq, binary narrowing | faultline-localization | — | Outcome correctness | Req 3.2 |
| `skipped_midpoint_yields_suspect_window` | Skip between boundaries yields SuspectWindow | 3-commit seq, mid=Skip | faultline-localization | — | Ambiguity detection | Req 3.3 |
| `indeterminate_midpoint_yields_suspect_window` | Indeterminate between boundaries yields SuspectWindow | 3-commit seq, mid=Indeterminate | faultline-localization | — | Ambiguity detection | Req 3.4 |
| `missing_pass_boundary_yields_inconclusive` | No Pass observation yields Inconclusive | Only Fail recorded | faultline-localization | — | Missing boundary | Req 3.6 |
| `missing_fail_boundary_yields_inconclusive` | No Fail observation yields Inconclusive | Only Pass recorded | faultline-localization | — | Missing boundary | Req 3.7 |
| `non_monotonic_evidence_yields_low_confidence` | Fail before Pass yields low confidence | 4-commit seq, non-monotonic | faultline-localization | — | Confidence scoring | Req 3.5 |
| `unobserved_between_boundaries_yields_inconclusive` | Unobserved commits between boundaries yields Inconclusive | 4-commit seq, gaps | faultline-localization | — | Incomplete evidence | — |
| `first_bad_with_all_between_observed` | All observed, clean transition yields FirstBad | 3-commit seq, all observed | faultline-localization | — | Happy path | — |
| `max_probes_exhausted_with_unobserved_between_boundaries` | Max probes + unobserved → Inconclusive with MaxProbesExhausted | max_probes=2, 5-commit seq | faultline-localization | — | Budget + incomplete | — |
| `max_probes_exhausted_with_skipped_between_boundaries` | Max probes + Skip → SuspectWindow with MaxProbesExhausted | max_probes=3, 3-commit seq | faultline-localization | — | Budget + ambiguity | — |
| `max_probes_exhausted_missing_pass_boundary` | Max probes + no Pass → Inconclusive with both reasons | max_probes=1, only Fail | faultline-localization | — | Budget + missing boundary | — |
| `max_probes_not_exhausted_no_extra_reason` | Converged within budget → no MaxProbesExhausted | 2-commit seq, default policy | faultline-localization | — | Clean convergence | — |
| `max_probes_exhausted_but_converged_yields_first_bad` | Converged exactly at budget → still FirstBad | max_probes=2, 2-commit seq | faultline-localization | — | Convergence priority | — |
| `prop_adjacent_pass_fail_yields_first_bad` | Adjacent Pass-Fail always yields FirstBad with high confidence | `n in 2..=20`, `i_frac` | faultline-localization | — | P5: Adjacent boundary | Req 3.2 |
| `prop_binary_narrowing_selects_valid_midpoint` | next_probe() returns a valid unobserved midpoint | `n in 3..=50` | faultline-localization | — | P4: Valid midpoint | Req 3.1 |
| `prop_first_bad_requires_direct_evidence` | FirstBad outcome has direct Pass and Fail observations at boundary | `n in 2..=20`, `i_frac` | faultline-localization | — | P10: Direct evidence | Req 3.9, 11.1 |
| `prop_non_monotonic_evidence_yields_low_confidence` | Non-monotonic evidence always yields low confidence | `n in 4..=20`, fill selectors | faultline-localization | — | P7: Low confidence | Req 3.5 |
| `prop_only_pass_yields_inconclusive_missing_fail_boundary` | All-Pass observations yield Inconclusive with MissingFailBoundary | `n in 3..=15`, random indices | faultline-localization | — | P8: Missing boundary | Req 3.6, 3.7 |
| `prop_only_fail_yields_inconclusive_missing_pass_boundary` | All-Fail observations yield Inconclusive with MissingPassBoundary | `n in 3..=15`, random indices | faultline-localization | — | P8: Missing boundary | Req 3.6, 3.7 |
| `prop_monotonic_window_narrowing` | Window never expands during monotonic binary narrowing | `n in 3..=20`, transition_frac | faultline-localization | — | P21: Monotonic narrowing | Req 11.2 |
| `prop_suspect_window_confidence_cap` | SuspectWindow confidence is always < high | `n in 4..=20`, ambig selectors | faultline-localization | — | P22: Confidence cap | Req 11.3 |
| `prop_ambiguous_observations_yield_suspect_window` | Skip/Indeterminate between boundaries yields SuspectWindow with correct reasons | `n in 4..=20`, class selectors | faultline-localization | — | P6: Ambiguous observations | Req 3.3, 3.4 |
| `prop_observation_order_independence` | Outcome is independent of observation recording order | `n in 3..=12`, permutation seeds | faultline-localization | — | P23: Order independence | Req 11.4 |
| `prop_max_probe_exhaustion_produces_explicit_outcome` | Max probes exhausted always includes MaxProbesExhausted reason | `n in 5..=30`, `max_probes in 2..=5` | faultline-localization | — | P25: Explicit exhaustion | Req 3.2 |
| `prop_observation_sequence_order_preservation` | observation_list() returns observations sorted by sequence_index | `n in 2..=20`, seq_indices | faultline-localization | — | P26: Order preservation | Req 3.3 |

#### faultline-localization — Fixture Scenarios

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs |
|----------|---------|-------------------|----------|----------|-----------|------|
| `fixture_skipped_midpoint_yields_suspect_window` | Skipped midpoint produces SuspectWindow with SkippedRevision | `RevisionSequenceBuilder::with_labels` | faultline-localization, faultline-fixtures | — | Fixture: skipped midpoint | Req 7.5 |
| `fixture_timed_out_midpoint_yields_suspect_window` | Timed-out midpoint produces SuspectWindow with IndeterminateRevision | `RevisionSequenceBuilder::with_labels` | faultline-localization, faultline-fixtures | — | Fixture: timed-out midpoint | Req 7.5 |
| `fixture_non_monotonic_yields_low_confidence` | Non-monotonic evidence produces low confidence SuspectWindow | `RevisionSequenceBuilder::with_labels` | faultline-localization, faultline-fixtures | — | Fixture: non-monotonic | Req 7.5 |

### faultline-surface

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs |
|----------|---------|-------------------|----------|----------|-----------|------|
| `bucket_name_extracts_top_level_directory` | Top-level directory extraction | Fixed paths | faultline-surface | — | Bucketing correctness | — |
| `bucket_name_root_level_file_uses_filename` | Root-level files use filename as bucket | Fixed paths | faultline-surface | — | Bucketing correctness | — |
| `bucket_name_empty_path_returns_repo_root` | Empty path maps to "repo-root" | Empty/slash paths | faultline-surface | — | Edge case | — |
| `surface_kind_source` | Source files classified as "source" | Fixed paths | faultline-surface | — | Classification | Req 5.2 |
| `surface_kind_tests` | Test files classified as "tests" | Fixed paths | faultline-surface | — | Classification | Req 5.2 |
| `surface_kind_benchmarks` | Benchmark files classified as "benchmarks" | Fixed paths | faultline-surface | — | Classification | — |
| `surface_kind_scripts` | Script files classified as "scripts" | Fixed paths | faultline-surface | — | Classification | — |
| `surface_kind_workflows` | Workflow files classified as "workflows" | Fixed paths | faultline-surface | — | Classification | — |
| `surface_kind_docs` | Doc files classified as "docs" | Fixed paths | faultline-surface | — | Classification | — |
| `surface_kind_build_script` | build.rs classified as "build-script" | Fixed paths | faultline-surface | — | Classification | — |
| `surface_kind_lockfile` | Lock files classified as "lockfile" | Fixed paths | faultline-surface | — | Classification | — |
| `surface_kind_migrations` | Migration files classified as "migrations" | Fixed paths | faultline-surface | — | Classification | — |
| `surface_kind_other` | Unrecognized files classified as "other" | Fixed paths | faultline-surface | — | Classification | — |
| `execution_surface_workflows` | Workflow files are execution surfaces | Fixed paths | faultline-surface | — | Execution surface | — |
| `execution_surface_build_scripts` | build.rs files are execution surfaces | Fixed paths | faultline-surface | — | Execution surface | — |
| `execution_surface_shell_scripts` | Shell scripts are execution surfaces | Fixed paths | faultline-surface | — | Execution surface | — |
| `non_execution_surface` | Source/config files are not execution surfaces | Fixed paths | faultline-surface | — | Negative case | — |
| `summarize_empty_input` | Empty input produces empty summary | Empty slice | faultline-surface | — | Edge case | — |
| `summarize_groups_by_top_level_directory` | Changes grouped by top-level directory | Mixed paths | faultline-surface | — | Grouping correctness | Req 5.3 |
| `summarize_collects_execution_surfaces` | Execution surfaces collected separately | Mixed paths | faultline-surface | — | Separation | Req 5.4 |
| `summarize_assigns_multiple_surface_kinds_per_bucket` | Buckets can have multiple surface kinds | Mixed paths in one dir | faultline-surface | — | Multi-kind buckets | — |
| `summarize_total_changes_equals_input_length` | total_changes matches input count | 5 mixed paths | faultline-surface | — | Count correctness | — |
| `summarize_every_path_in_exactly_one_bucket` | Every path appears in exactly one bucket | 4 mixed paths | faultline-surface | — | Partition correctness | — |
| `prop_surface_analysis_invariants` | Surface analysis maintains all structural invariants | `arb_path_change()` × 1..30 | faultline-surface | — | P13: Surface invariants | Req 5.2, 5.3, 5.4 |

---

## Adapter Tier — BDD, Property, and Golden Tests

### faultline-probe-exec

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs |
|----------|---------|-------------------|----------|----------|-----------|------|
| `prop_exit_code_classification` | Exit codes map to correct ObservationClass | `arb exit_code × timed_out` | faultline-probe-exec | — | P1: Exit code classification | Req 2.3–2.6 |
| `prop_signal_aware_exit_code_classification` | Signal-aware classification handles all combinations | `arb exit_code × timed_out × signal_number` | faultline-probe-exec | — | P27: Signal-aware classification | Req 5.1, 5.2 |
| `prop_observation_structural_completeness` | Probe observations contain all required fields | `arb_commit_id() × arb_probe_kind()` | faultline-probe-exec | — | P2/P28: Structural completeness | Req 2.7, 4.7, 5.4 |
| `truncate_output_under_limit_unchanged` | Output under limit is unchanged | Fixed short string | faultline-probe-exec | — | Truncation correctness | — |
| `truncate_output_at_limit_unchanged` | Output exactly at limit is unchanged | String of limit length | faultline-probe-exec | — | Truncation boundary | — |
| `truncate_output_over_limit_truncated` | Output over limit is truncated with marker | String exceeding limit | faultline-probe-exec | — | Truncation correctness | — |
| `truncate_output_empty_string` | Empty string is unchanged | Empty string | faultline-probe-exec | — | Edge case | — |
| `signal_termination_sets_signal_number` | Signal-killed process sets signal_number field | Shell probe with self-SIGTERM | faultline-probe-exec | — | Signal detection | Req 5.1 |
| `probe_output_exceeding_limit_is_truncated` | Real probe output exceeding limit is truncated | Shell probe with large output | faultline-probe-exec | — | Integration truncation | — |

### faultline-store

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs |
|----------|---------|-------------------|----------|----------|-----------|------|
| `prepare_run_creates_directory_and_request_json` | Run preparation creates directory and persists request | Sample request + TempDir | faultline-store | request.json | Persistence | — |
| `prepare_run_sets_resumed_on_second_call` | Second prepare_run sets resumed=true | Same request twice | faultline-store | — | Resume detection | — |
| `load_observations_returns_empty_when_no_file` | Missing observations file returns empty vec | Fresh run | faultline-store | — | Default behavior | — |
| `save_and_load_single_observation` | Single observation round-trips through store | Sample observation + TempDir | faultline-store | observations.json | Round-trip | — |
| `save_observation_upserts_by_commit_id` | Saving same commit replaces previous observation | Two observations, same commit | faultline-store | observations.json | Upsert semantics | — |
| `save_observation_sorts_by_sequence_index` | Observations sorted by sequence_index on disk | Out-of-order observations | faultline-store | observations.json | Sort order | — |
| `save_and_read_report` | Report round-trips through store | Sample report + TempDir | faultline-store | report.json | Round-trip | — |
| `load_report_returns_none_when_no_file` | Missing report file returns None | Fresh run | faultline-store | — | Default behavior | — |
| `load_report_returns_saved_report` | Saved report can be loaded back | Sample report + TempDir | faultline-store | report.json | Round-trip | — |
| `new_creates_root_directory` | FileRunStore::new creates nested directories | Deep nested path | faultline-store | — | Directory creation | — |
| `prepare_run_creates_lock_file` | Lock file created with PID and timestamp | Sample request + TempDir | faultline-store | .lock | Lock creation | — |
| `prepare_run_allows_same_process_reentry` | Same process can re-acquire lock | Same request twice | faultline-store | .lock | Re-entry | — |
| `prepare_run_rejects_lock_held_by_another_live_process` | Lock held by live process is rejected (Unix) | Lock with PID 1 | faultline-store | — | Lock enforcement | — |
| `prepare_run_cleans_stale_lock_from_dead_process` | Stale lock from dead process is cleaned | Lock with high PID | faultline-store | .lock | Stale lock cleanup | — |
| `save_report_releases_lock` | Lock released after saving report | Full run lifecycle | faultline-store | — | Lock release | — |
| `atomic_write_produces_correct_content_and_no_tmp_file` | Atomic write produces correct content, no .tmp residue | Fixed content + TempDir | faultline-store | — | Atomic write | — |
| `save_probe_logs_creates_log_files` | Probe logs saved to per-commit files | Sample logs + TempDir | faultline-store | logs/*.log | Log persistence | — |
| `save_probe_logs_creates_logs_directory` | Logs directory created on first save | Fresh run | faultline-store | logs/ | Directory creation | — |
| `save_probe_logs_overwrites_existing_logs` | Re-saving logs overwrites previous content | Two saves, same commit | faultline-store | logs/*.log | Overwrite semantics | — |
| `save_probe_logs_handles_multiple_commits` | Multiple commits have separate log files | Two different commits | faultline-store | logs/*.log | Isolation | — |

### faultline-render

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs |
|----------|---------|-------------------|----------|----------|-----------|------|
| `render_writes_analysis_json` | Render produces analysis.json | Sample report + TempDir | faultline-render | analysis.json | File creation | Req 6.1 |
| `analysis_json_contains_all_fields` | analysis.json has all required top-level fields | Sample report | faultline-render | analysis.json | Field completeness | Req 6.2 |
| `analysis_json_is_deterministic` | Same report produces identical JSON | Sample report × 2 dirs | faultline-render | analysis.json | Determinism | Req 6.3 |
| `analysis_json_is_valid_json` | analysis.json is valid JSON | Sample report | faultline-render | analysis.json | Validity | Req 6.4 |
| `render_creates_output_directory` | Render creates output directory if missing | Non-existent dir | faultline-render | — | Directory creation | — |
| `output_dir_returns_configured_path` | output_dir() accessor works | Fixed path | faultline-render | — | Accessor | — |
| `analysis_json_round_trips` | JSON round-trip preserves report equality | Sample report | faultline-render | analysis.json | Round-trip | — |
| `render_writes_index_html` | Render produces index.html | Sample report + TempDir | faultline-render | index.html | File creation | Req 7.1 |
| `html_contains_run_id` | HTML contains the run ID | Sample report | faultline-render | index.html | Content | Req 7.2 |
| `html_contains_first_bad_outcome` | HTML shows FirstBad outcome details | FirstBad report | faultline-render | index.html | Content | Req 7.2 |
| `html_contains_suspect_window_outcome` | HTML shows SuspectWindow outcome details | SuspectWindow report | faultline-render | index.html | Content | Req 7.2 |
| `html_contains_inconclusive_outcome` | HTML shows Inconclusive outcome | Inconclusive report | faultline-render | index.html | Content | Req 7.2 |
| `html_contains_probe_fingerprint_and_history_mode` | HTML contains probe fingerprint and history mode | Sample report | faultline-render | index.html | Content | Req 7.2 |
| `html_contains_observation_timeline_rows` | HTML has correct number of observation rows | Sample report (2 obs) | faultline-render | index.html | Row count | Req 7.2 |
| `html_contains_surface_buckets` | HTML contains surface bucket information | Sample report | faultline-render | index.html | Content | Req 7.2 |
| `html_contains_changed_paths` | HTML contains changed path information | Sample report | faultline-render | index.html | Content | Req 7.2 |
| `html_has_no_external_dependencies` | HTML has no external links, scripts, or images | Sample report | faultline-render | index.html | Self-contained | Req 7.3 |
| `html_has_inline_css` | HTML has inline `<style>` block | Sample report | faultline-render | index.html | Self-contained | Req 7.3 |
| `escape_html_replaces_special_chars` | HTML escaping handles all special characters | XSS-like input | faultline-render | — | Security | Req 7.5 |
| `html_escapes_dynamic_content` | Dynamic content in HTML is escaped | Report with `<script>` in run_id | faultline-render | index.html | Security | Req 7.5 |
| `html_has_valid_structure` | HTML has doctype, html, head, body | Sample report | faultline-render | index.html | Structure | Req 7.1 |
| `html_has_outcome_firstbad_class` | FirstBad uses outcome-firstbad CSS class | FirstBad report | faultline-render | index.html | Visual distinction | Req 8.8 |
| `html_has_outcome_suspect_class` | SuspectWindow uses outcome-suspect CSS class | SuspectWindow report | faultline-render | index.html | Visual distinction | Req 8.8 |
| `html_has_outcome_inconclusive_class` | Inconclusive uses outcome-inconclusive CSS class | Inconclusive report | faultline-render | index.html | Visual distinction | Req 8.8 |
| `html_renders_ambiguity_badges_suspect_window` | SuspectWindow renders reason badges | SuspectWindow with reasons | faultline-render | index.html | Badges | Req 8.9 |
| `html_renders_ambiguity_badges_inconclusive` | Inconclusive renders reason badges | Inconclusive with reasons | faultline-render | index.html | Badges | Req 8.9 |
| `html_observations_sorted_by_sequence_index` | Observations sorted by sequence_index in HTML | Report with reversed indices | faultline-render | index.html | Sort order | Req 8.10 |
| `golden_analysis_json` | Golden snapshot of canonical analysis.json | `canonical_fixture_report()` | faultline-render | analysis.json snapshot | Golden contract | Req 3.4 |
| `golden_index_html` | Golden snapshot of canonical index.html | `canonical_fixture_report()` | faultline-render | index.html snapshot | Golden contract | Req 3.4 |
| `prop_html_contains_required_data` | HTML contains all required data from report | `arb_analysis_report()` | faultline-render | index.html | P16: Required data | Req 7.2 |
| `prop_html_escaping_correctness` | HTML escaping prevents injection | Arbitrary strings with special chars | faultline-render | — | P17: Escaping | Req 7.5 |
| `prop_html_is_self_contained` | HTML has no external dependencies | `arb_analysis_report()` | faultline-render | index.html | P18: Self-contained | Req 7.3 |
| `prop_html_outcome_visual_distinction_and_badges` | Outcome CSS classes and badges are correct | `arb_analysis_report()` | faultline-render | index.html | P29: Visual distinction | Req 8.8, 8.9 |
| `prop_html_temporal_observation_order` | Observations in HTML sorted by sequence_index | `arb_analysis_report()` (≥2 obs) | faultline-render | index.html | P30: Temporal order | Req 8.10 |
| `prop_html_execution_surface_separation` | Execution surfaces rendered separately | `arb_analysis_report()` (non-empty exec) | faultline-render | index.html | P31: Surface separation | Req 8.11 |

### faultline-sarif

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs |
|----------|---------|-------------------|----------|----------|-----------|------|
| `sarif_first_bad_produces_error_level` | FirstBad maps to SARIF error level | FirstBad report | faultline-sarif | SARIF JSON | Level mapping | Req 3.6 |
| `sarif_suspect_window_produces_warning_level` | SuspectWindow maps to SARIF warning level | SuspectWindow report | faultline-sarif | SARIF JSON | Level mapping | Req 3.6 |
| `sarif_inconclusive_produces_note_level` | Inconclusive maps to SARIF note level | Inconclusive report | faultline-sarif | SARIF JSON | Level mapping | Req 3.6 |
| `sarif_empty_changed_paths_produces_empty_locations` | Empty changed_paths produces empty locations | Report with no paths | faultline-sarif | SARIF JSON | Edge case | Req 3.6 |
| `prop_sarif_export_structural_validity` | SARIF output is structurally valid for all reports | `arb_analysis_report()` | faultline-sarif | SARIF JSON | P41: Structural validity | Req 3.6 |

### faultline-junit

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs |
|----------|---------|-------------------|----------|----------|-----------|------|
| `junit_first_bad_has_failure_element` | FirstBad produces `<failure>` element | FirstBad report | faultline-junit | JUnit XML | Element mapping | Req 3.7 |
| `junit_suspect_window_has_failure_element` | SuspectWindow produces `<failure>` element | SuspectWindow report | faultline-junit | JUnit XML | Element mapping | Req 3.7 |
| `junit_inconclusive_has_failure_element` | Inconclusive produces `<failure>` element | Inconclusive report | faultline-junit | JUnit XML | Element mapping | Req 3.7 |
| `junit_observations_in_system_out` | Observations listed in `<system-out>` | FirstBad report with observations | faultline-junit | JUnit XML | Content | Req 3.7 |
| `junit_empty_observations` | Empty observations handled gracefully | Report with no observations | faultline-junit | JUnit XML | Edge case | Req 3.7 |
| `prop_junit_xml_export_structural_validity` | JUnit XML is structurally valid for all reports | `arb_analysis_report()` | faultline-junit | JUnit XML | P42: Structural validity | Req 3.7 |

---

## App-Integration Tier

### faultline-app

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs |
|----------|---------|-------------------|----------|----------|-----------|------|
| `integration_cached_resume_skips_cached_commits` | Cached observations are reused on resume | Mock ports, 5-commit seq | faultline-app | — | Resume correctness | — |
| `integration_good_boundary_fail_yields_invalid_boundary` | Good boundary returning Fail is rejected | Mock ports, 5-commit seq | faultline-app | — | Boundary validation | Req 10.1–10.5 |
| `integration_bad_boundary_pass_yields_invalid_boundary` | Bad boundary returning Pass is rejected | Mock ports, 5-commit seq | faultline-app | — | Boundary validation | Req 10.1–10.5 |
| `integration_cached_boundary_observations_reused_no_reprobe` | Cached boundary observations are not re-probed | TrackingProbe, 5-commit seq | faultline-app | — | Cache efficiency | — |
| `integration_full_localization_loop_with_mock_ports` | Full localization loop produces correct report | Mock ports, 10-commit seq | faultline-app | AnalysisReport | End-to-end correctness | — |
| `prop_probe_count_respects_max_probes` | Probe count never exceeds max_probes | `max_probes in 1..=10`, 20-commit seq | faultline-app | — | P9: Budget enforcement | Req 3.8 |
| `prop_good_boundary_fail_yields_invalid_boundary` | Good boundary Fail always yields InvalidBoundary | `n in 3..=20` | faultline-app | — | P20: Boundary validation | Req 10.1–10.4 |
| `prop_bad_boundary_pass_yields_invalid_boundary` | Bad boundary Pass always yields InvalidBoundary | `n in 3..=20` | faultline-app | — | P20: Boundary validation | Req 10.1–10.4 |

---

## CLI Smoke Tier

### faultline-cli

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs |
|----------|---------|-------------------|----------|----------|-----------|------|
| `rejects_both_cmd_and_program` | --cmd and --program are mutually exclusive | CLI validation | faultline-cli | — | Input validation | — |
| `rejects_neither_cmd_nor_program` | One of --cmd or --program is required | CLI validation | faultline-cli | — | Input validation | — |
| `help_output_describes_all_flags` | --help lists all expected flags | `Cli::command()` | faultline-cli | — | Help completeness | — |
| `golden_cli_help` | Golden snapshot of CLI --help text | `Cli::command().write_long_help()` | faultline-cli | help snapshot | Golden contract | Req 3.4 |
| `exit_code_0_for_first_bad` | FirstBad maps to exit code 0 | Hand-built outcome | faultline-cli | — | Exit code mapping | — |
| `exit_code_1_for_suspect_window` | SuspectWindow maps to exit code 1 | Hand-built outcome | faultline-cli | — | Exit code mapping | — |
| `exit_code_3_for_inconclusive` | Inconclusive maps to exit code 3 | Hand-built outcome | faultline-cli | — | Exit code mapping | — |
| `exit_code_2_for_execution_error` | ExecutionError maps to exit code 2 | OperatorCode | faultline-cli | — | Exit code mapping | — |
| `exit_code_4_for_invalid_input` | InvalidInput maps to exit code 4 | OperatorCode | faultline-cli | — | Exit code mapping | — |
| `all_exit_codes_are_distinct` | All exit codes are unique | All OperatorCodes | faultline-cli | — | Uniqueness | — |
| `rejects_resume_and_force` | --resume and --force are mutually exclusive | CLI validation | faultline-cli | — | Input validation | — |
| `rejects_resume_and_fresh` | --resume and --fresh are mutually exclusive | CLI validation | faultline-cli | — | Input validation | — |
| `rejects_force_and_fresh` | --force and --fresh are mutually exclusive | CLI validation | faultline-cli | — | Input validation | — |
| `accepts_single_run_modes` | Individual run modes accepted | CLI validation | faultline-cli | — | Happy path | — |
| `accepts_valid_env_vars` | Valid KEY=VALUE env vars accepted | CLI validation | faultline-cli | — | Happy path | — |
| `accepts_env_var_with_equals_in_value` | Env var with = in value accepted | CLI validation | faultline-cli | — | Edge case | — |
| `rejects_env_var_missing_equals` | Env var without = rejected | CLI validation | faultline-cli | — | Input validation | — |
| `accepts_empty_env_list` | Empty env list accepted | CLI validation | faultline-cli | — | Edge case | — |
| `accepts_valid_shell_kinds` | Valid shell kinds accepted | CLI validation | faultline-cli | — | Happy path | — |
| `accepts_no_shell` | No --shell is accepted | CLI validation | faultline-cli | — | Happy path | — |
| `rejects_unknown_shell` | Unknown shell kind rejected | CLI validation | faultline-cli | — | Input validation | — |
| `prop_operator_code_exit_code_mapping` | Exit code mapping is correct for all outcomes | `arb_localization_outcome()` | faultline-cli | — | P32: Exit code mapping | Req 8.1 |
| `prop_cli_help_flag_completeness` | CLI --help lists all flags | `any::<u32>()` (seed) | faultline-cli | — | P36: Help completeness | Req 9.6 |
| `smoke_cli_produces_artifacts` | Full CLI run produces analysis.json and index.html | `GitRepoBuilder` 3-commit repo | faultline-cli, faultline-app, all adapters | analysis.json, index.html | End-to-end smoke | — |

---

## Tooling Tier

### xtask

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs |
|----------|---------|-------------------|----------|----------|-----------|------|
| `xtask_help_lists_all_subcommands` | xtask --help lists all subcommands | `Cli::command()` | xtask | — | P43: Help completeness | Req 5.2, 5.5 |
| `scaffold_help_lists_all_kinds` | scaffold --help lists all scaffold kinds | `Cli::command()` | xtask | — | P43: Help completeness | Req 10.1 |
| `prop_schema_drift_detection` | Schema drift is detected when schema file differs | Generated schema + modified file | xtask, faultline-types | — | P45: Schema drift | Req 8.3 |
| `schema_drift_error_message_format` | Schema drift error message has correct format | Modified schema file | xtask | — | Error message format | Req 8.3 |
| `pattern_entry_structural_completeness` | All pattern entries have required sections | `docs/patterns/catalog.md` | xtask | — | P37: Pattern completeness | Req 1.2 |
| `scenario_entry_structural_completeness` | All scenario entries have 7 required fields | `docs/scenarios/scenario_index.md` | xtask | — | P38: Scenario completeness | Req 2.2 |
| `extract_test_names_finds_standard_tests` | Test name extraction finds `#[test]` functions | Hand-built source | xtask | — | Extraction correctness | Req 2.5 |
| `extract_test_names_finds_proptest_fns` | Test name extraction finds proptest functions | Hand-built source | xtask | — | Extraction correctness | Req 2.5 |
| `extract_index_entries_parses_table` | Index entry extraction parses markdown tables | Hand-built index | xtask | — | Extraction correctness | Req 2.5 |
| `check_consistency_reports_symmetric_difference` | Consistency check reports missing and stale entries | Hand-built sets | xtask | — | Symmetric difference | Req 2.5, 8.4 |
| `check_consistency_perfect_match_is_ok` | Perfect match yields ok result | Identical sets | xtask | — | Happy path | Req 2.5 |
| `prop_symmetric_difference_is_exact` | Symmetric difference is exact for random sets | `arb_test_name_set()` | xtask | — | P39: Atlas consistency | Req 2.5, 8.4 |
| `prop_identical_sets_yield_ok` | Identical sets always yield ok | `arb_test_name_set()` | xtask | — | P39: Atlas consistency | Req 2.5, 8.4 |
| `prop_extract_and_check_round_trip` | Extract + check round-trips for matching names | Random test names | xtask | — | P39: Atlas consistency | Req 2.5, 8.4 |
| `tool_detection_error_messages_known_tools` | Known tool error messages contain name and install cmd | Known tool list | xtask | — | P44: Tool detection | Req 5.7 |
| `tool_detection_error_message_format` | Tool error message format is correct for arbitrary tools | `arb name × install_cmd` | xtask | — | P44: Tool detection | Req 5.7 |
| `ci_failure_messages_identify_broken_contract` | CI failure messages identify broken contract | Known contracts | xtask | — | P46: CI messages | Req 8.7 |
| `ci_contract_broken_message_contains_contract_name` | Contract broken message contains contract name | `arb contract name` | xtask | — | P46: CI messages | Req 8.7 |
| `ci_golden_failure_message_contains_artifact_and_docs` | Golden failure message contains artifact and docs ref | `arb artifact name` | xtask | — | P46: CI messages | Req 8.7 |
| `ci_missing_scenario_message_contains_files_and_docs` | Missing scenario message contains files and docs ref | `arb file names` | xtask | — | P46: CI messages | Req 8.7 |
| `prop_scaffold_crate_generation` | Scaffold crate generates valid structure | `arb_crate_suffix() × arb_tier()` | xtask | Cargo.toml, lib.rs | P47: Scaffold crate | Req 10.2 |
| `prop_scaffold_adr_sequential_numbering` | Scaffold ADR uses next sequential number | `existing_count in 0..20` | xtask | ADR file | P48: ADR numbering | Req 10.3 |
| `prop_scaffold_scenario_creates_stub_and_index` | Scaffold scenario creates test stub and index entry | `arb scenario name` | xtask | test stub, index entry | P49: Scaffold files | Req 10.4, 10.5 |
| `prop_scaffold_doc_creates_file_and_summary_entry` | Scaffold doc creates file and SUMMARY.md entry | `arb section` | xtask | doc file, SUMMARY entry | P49: Scaffold files | Req 10.4, 10.5 |
| `prop_scaffold_rejects_invalid_crate_names` | Invalid crate names are rejected | Invalid name patterns | xtask | — | P50: Input validation | Req 10.6 |
| `prop_scaffold_rejects_empty_adr_titles` | Empty ADR titles are rejected | Empty/whitespace strings | xtask | — | P50: Input validation | Req 10.6 |
| `prop_scaffold_rejects_empty_scenario_names` | Empty scenario names are rejected | Empty/whitespace strings | xtask | — | P50: Input validation | Req 10.6 |
| `prop_scaffold_rejects_invalid_doc_sections` | Invalid doc sections are rejected | Invalid section names | xtask | — | P50: Input validation | Req 10.6 |

---

## Testing Framework by Tier

| Tier | Framework | Notes |
|------|-----------|-------|
| Domain (property tests) | `proptest` | 100 cases per property, pure logic, no I/O |
| Domain (unit tests) | `#[test]` | Fixture-based scenarios with `make_seq`, `RevisionSequenceBuilder` |
| Adapter (BDD/integration) | `#[test]` + `tempfile` | Real filesystem, real processes |
| Adapter (property tests) | `proptest` | 100 cases, may use `tempfile` for I/O |
| App-integration | `#[test]` + mock ports | Mock implementations of port traits |
| CLI smoke | `std::process::Command` | Builds real Git repo via `GitRepoBuilder`, runs CLI binary |
| Golden tests | `insta` | Snapshot testing for artifacts and help text |
