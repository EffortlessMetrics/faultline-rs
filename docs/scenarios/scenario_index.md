# Scenario Index

This index catalogs every BDD scenario and property test in the faultline workspace. Each entry includes the scenario name, problem description, fixture or generator, crate(s) exercised, artifact(s) produced or validated, invariant or property asserted, related references, and enriched metadata (scenario tier, requirement IDs, artifact contract, mutation surface, criticality, ownership hint, human review required).

Scenarios are organized by crate tier following the verification matrix.

---

## Domain Tier — Property Tests

### faultline-codes

_No dedicated tests. Enum types are exercised transitively through `faultline-types` and `faultline-localization` tests._

### faultline-types

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `all_types_derive_required_traits` | All public types implement Serialize, Deserialize, Debug, Clone, PartialEq, Eq | Hand-built samples | faultline-types | — | Trait completeness | — | domain | — | — | faultline-types | P1 | faultline-types | no |
| `stable_hash_deterministic` | FNV hash produces identical output for identical input | Fixed input `"hello world"` | faultline-types | — | Determinism | — | domain | — | — | faultline-types | P1 | faultline-types | no |
| `stable_hash_different_inputs_differ` | Different inputs produce different hashes | Fixed inputs `"hello"`, `"world"` | faultline-types | — | Collision resistance | — | domain | — | — | faultline-types | P1 | faultline-types | no |
| `stable_hash_returns_16_hex_chars` | Hash output is exactly 16 hex characters | Fixed input `"test"` | faultline-types | — | Format correctness | — | domain | — | — | faultline-types | P2 | faultline-types | no |
| `now_epoch_seconds_returns_reasonable_value` | Timestamp is within a plausible range | System clock | faultline-types | — | Sanity check | — | domain | — | — | faultline-types | P2 | faultline-types | no |
| `probe_spec_fingerprint_deterministic` | ProbeSpec fingerprint is deterministic | Sample ProbeSpec | faultline-types | — | Determinism | — | domain | — | — | faultline-types | P1 | faultline-types | no |
| `probe_spec_fingerprint_differs_for_different_specs` | Different ProbeSpecs produce different fingerprints | Two distinct ProbeSpecs | faultline-types | — | Uniqueness | — | domain | — | — | faultline-types | P1 | faultline-types | no |
| `analysis_request_fingerprint_deterministic` | AnalysisRequest fingerprint is deterministic | Sample AnalysisRequest | faultline-types | — | Determinism | — | domain | — | — | faultline-types | P1 | faultline-types | no |
| `analysis_request_fingerprint_differs_for_different_requests` | Different requests produce different fingerprints | Two distinct AnalysisRequests | faultline-types | — | Uniqueness | — | domain | — | — | faultline-types | P1 | faultline-types | no |
| `boundary_pair_first_bad` | FirstBad outcome returns correct boundary pair | Hand-built outcome | faultline-types | — | Accessor correctness | — | domain | — | — | faultline-types | P1 | faultline-types | no |
| `boundary_pair_suspect_window` | SuspectWindow outcome returns correct boundary pair | Hand-built outcome | faultline-types | — | Accessor correctness | — | domain | — | — | faultline-types | P1 | faultline-types | no |
| `boundary_pair_inconclusive_returns_none` | Inconclusive outcome returns None for boundary pair | Hand-built outcome | faultline-types | — | Accessor correctness | — | domain | — | — | faultline-types | P1 | faultline-types | no |
| `confidence_high` | Confidence::high() returns score=95, label="high" | Constructor | faultline-types | — | Value correctness | — | domain | — | — | faultline-types | P2 | faultline-types | no |
| `confidence_medium` | Confidence::medium() returns score=65, label="medium" | Constructor | faultline-types | — | Value correctness | — | domain | — | — | faultline-types | P2 | faultline-types | no |
| `confidence_low` | Confidence::low() returns score=25, label="low" | Constructor | faultline-types | — | Value correctness | — | domain | — | — | faultline-types | P2 | faultline-types | no |
| `search_policy_default` | Default SearchPolicy has max_probes=64 | Default constructor | faultline-types | — | Default correctness | — | domain | — | — | faultline-types | P2 | faultline-types | no |
| `commit_id_display` | CommitId Display impl formats correctly | Fixed CommitId | faultline-types | — | Display correctness | — | domain | — | — | faultline-types | P2 | faultline-types | no |
| `revision_sequence_len_and_is_empty` | RevisionSequence len/is_empty work correctly | Empty and non-empty sequences | faultline-types | — | Accessor correctness | — | domain | — | — | faultline-types | P2 | faultline-types | no |
| `faultline_error_from_io` | FaultlineError::from(io::Error) produces Io variant | io::Error | faultline-types | — | Error conversion | — | domain | — | — | faultline-types | P2 | faultline-types | no |
| `faultline_error_from_serde_json` | FaultlineError::from(serde_json::Error) produces Serde variant | Invalid JSON | faultline-types | — | Error conversion | — | domain | — | — | faultline-types | P2 | faultline-types | no |
| `faultline_error_display` | All FaultlineError variants display correctly | Each variant | faultline-types | — | Display correctness | — | domain | — | — | faultline-types | P2 | faultline-types | no |
| `prop_json_serialization_determinism` | Serializing the same AnalysisReport twice produces byte-identical JSON | `arb_analysis_report()` | faultline-types | — | P14: Determinism | Req 6.3 | domain | Req 6.3 | analysis.json | faultline-types | P0 | faultline-types | yes |
| `prop_analysis_report_json_round_trip` | JSON round-trip preserves equality for AnalysisReport | `arb_analysis_report()` | faultline-types | — | P15: Round-trip | Req 6.5 | domain | Req 6.5 | analysis.json | faultline-types | P0 | faultline-types | yes |
| `prop_json_schema_validates_all_valid_reports` | Generated JSON Schema accepts all valid AnalysisReport instances | `arb_analysis_report()` | faultline-types | — | P40: Schema validity | Req 3.1, 3.2 | domain | Req 3.1, 3.2 | analysis-report.schema.json | faultline-types | P0 | faultline-types | yes |
| `flake_policy_default` | Default FlakePolicy has retries=0, stability_threshold=1.0 | Default constructor | faultline-types | — | Default correctness | — | domain | Req 3.6 | — | faultline-types | P2 | faultline-types | no |
| `to_shell_script_exec_variant` | Shell script for Exec probe contains required fields | Hand-built capsule | faultline-types | — | Script correctness | — | domain | Req 4.4 | — | faultline-types | P1 | faultline-types | no |
| `to_shell_script_shell_variant` | Shell script for Shell probe contains required fields | Hand-built capsule | faultline-types | — | Script correctness | — | domain | Req 4.4 | — | faultline-types | P1 | faultline-types | no |
| `to_shell_script_escapes_single_quotes` | Shell script escapes single quotes in arguments | Hand-built capsule with quotes | faultline-types | — | Security | — | domain | Req 4.4 | — | faultline-types | P1 | faultline-types | no |
| `compare_runs_self_comparison_yields_zero_diff` | Self-comparison produces zero diff | Sample report | faultline-types | — | Identity comparison | — | domain | Req 5.3 | — | faultline-types | P1 | faultline-types | no |
| `compare_runs_different_outcomes` | Different outcomes detected | Two distinct reports | faultline-types | — | Outcome change detection | — | domain | Req 5.1 | — | faultline-types | P1 | faultline-types | no |
| `compare_runs_suspect_path_diffs` | Suspect path additions/removals detected | Reports with different suspect surfaces | faultline-types | — | Path diff correctness | — | domain | Req 5.2 | — | faultline-types | P1 | faultline-types | no |
| `compare_runs_never_panics_on_empty_reports` | Empty reports compared without panic | Minimal empty reports | faultline-types | — | Totality | — | domain | Req 5.1 | — | faultline-types | P1 | faultline-types | no |
| `schema_evolution_old_version_deserializes_with_defaults` | Old v0.1.0 report deserializes with serde defaults | Hand-built v0.1.0 JSON | faultline-types | — | Forward compatibility | — | domain | — | analysis.json | faultline-types | P0 | faultline-types | yes |
| `prop_flake_signal_stability_classification` | FlakeSignal stability matches threshold logic, counts sum to total_runs | `arb ObservationClass vec × threshold` | faultline-types | — | P48: FlakeSignal stability | Req 3.2, 3.3 | domain | Req 3.2, 3.3 | — | faultline-types | P0 | faultline-types | yes |
| `prop_shell_script_contains_required_fields` | Shell script contains commit, predicate, timeout, env vars | `arb_reproduction_capsule()` | faultline-types | — | P52: Shell script generation | Req 4.4 | domain | Req 4.4 | — | faultline-types | P0 | faultline-types | yes |
| `prop_reproduction_capsule_structural_correspondence` | Capsule count equals observation count, fields match | `arb_report_with_capsules()` | faultline-types | — | P51: Capsule correspondence | Req 4.1, 4.2 | domain | Req 4.1, 4.2 | — | faultline-types | P0 | faultline-types | yes |
| `prop_compare_runs_is_total` | compare_runs never panics for any two reports | `arb_analysis_report() × 2` | faultline-types | — | P53: Totality | Req 5.1 | domain | Req 5.1 | — | faultline-types | P0 | faultline-types | yes |
| `prop_self_comparison_yields_zero_diff` | Self-comparison yields zero diff for any report | `arb_analysis_report()` | faultline-types | — | P54: Self-comparison | Req 5.3 | domain | Req 5.3 | — | faultline-types | P0 | faultline-types | yes |

### faultline-localization

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `new_rejects_empty_sequence` | Empty revision sequence is rejected | Empty RevisionSequence | faultline-localization | — | Input validation | — | domain | — | — | faultline-localization | P1 | faultline-localization | no |
| `new_builds_index_by_commit` | Session builds correct commit→index mapping | `make_seq(["a","b","c"])` | faultline-localization | — | Index correctness | — | domain | — | — | faultline-localization | P1 | faultline-localization | no |
| `record_rejects_unknown_commit` | Recording observation for unknown commit fails | `make_seq(["a","b"])` + unknown "z" | faultline-localization | — | Input validation | — | domain | — | — | faultline-localization | P1 | faultline-localization | no |
| `record_accepts_known_commit` | Recording observation for known commit succeeds | `make_seq(["a","b"])` | faultline-localization | — | Happy path | — | domain | — | — | faultline-localization | P2 | faultline-localization | no |
| `has_observation_and_get_observation` | Observation accessors work before and after recording | `make_seq(["a","b","c"])` | faultline-localization | — | Accessor correctness | — | domain | — | — | faultline-localization | P2 | faultline-localization | no |
| `observation_list_returns_all_in_sequence_index_order` | observation_list() sorts by sequence_index | `make_seq` + obs_with_seq | faultline-localization | — | Sort order | — | domain | — | — | faultline-localization | P1 | faultline-localization | no |
| `observation_list_orders_by_sequence_index_not_revision_position` | Sort is by sequence_index, not revision position | 5-commit sequence, binary-search order | faultline-localization | — | Sort order | — | domain | — | — | faultline-localization | P1 | faultline-localization | no |
| `record_preserves_preassigned_sequence_index` | Pre-assigned sequence_index is preserved | `obs_with_seq("a", Pass, 42)` | faultline-localization | — | Value preservation | — | domain | — | — | faultline-localization | P2 | faultline-localization | no |
| `sequence_accessor` | sequence() returns the original RevisionSequence | `make_seq(["a","b"])` | faultline-localization | — | Accessor correctness | — | domain | — | — | faultline-localization | P2 | faultline-localization | no |
| `max_probes_accessor` | max_probes() returns the policy value | Policy with max_probes=42 | faultline-localization | — | Accessor correctness | — | domain | — | — | faultline-localization | P2 | faultline-localization | no |
| `next_probe_probes_first_boundary_first` | First probe targets the first commit | `make_seq(["a","b","c"])` | faultline-localization | — | Boundary-first probing | — | domain | Req 3.1 | — | faultline-localization | P0 | faultline-localization | no |
| `next_probe_probes_last_boundary_second` | Second probe targets the last commit | `make_seq` + Pass at first | faultline-localization | — | Boundary-first probing | — | domain | Req 3.1 | — | faultline-localization | P0 | faultline-localization | no |
| `next_probe_binary_narrows_between_boundaries` | After boundaries, probe targets median unobserved | 5-commit seq, Pass/Fail at ends | faultline-localization | — | Binary narrowing | — | domain | Req 3.1 | — | faultline-localization | P0 | faultline-localization | no |
| `next_probe_returns_none_when_converged` | No more probes when adjacent pass-fail found | 2-commit seq, Pass+Fail | faultline-localization | — | Convergence detection | — | domain | Req 3.2 | — | faultline-localization | P0 | faultline-localization | no |
| `next_probe_respects_max_probes` | Stops probing when max_probes reached | max_probes=2, 5-commit seq | faultline-localization | — | Budget enforcement | — | domain | Req 3.8 | — | faultline-localization | P0 | faultline-localization | no |
| `next_probe_single_element_returns_none` | Single-element sequence returns None | 1-commit seq | faultline-localization | — | Edge case | — | domain | — | — | faultline-localization | P2 | faultline-localization | no |
| `exact_boundary_when_adjacent` | Adjacent Pass-Fail yields FirstBad | 3-commit seq, binary narrowing | faultline-localization | — | Outcome correctness | Req 3.2 | domain | Req 3.2 | — | faultline-localization | P0 | faultline-localization | no |
| `skipped_midpoint_yields_suspect_window` | Skip between boundaries yields SuspectWindow | 3-commit seq, mid=Skip | faultline-localization | — | Ambiguity detection | Req 3.3 | domain | Req 3.3 | — | faultline-localization | P0 | faultline-localization | no |
| `indeterminate_midpoint_yields_suspect_window` | Indeterminate between boundaries yields SuspectWindow | 3-commit seq, mid=Indeterminate | faultline-localization | — | Ambiguity detection | Req 3.4 | domain | Req 3.4 | — | faultline-localization | P0 | faultline-localization | no |
| `missing_pass_boundary_yields_inconclusive` | No Pass observation yields Inconclusive | Only Fail recorded | faultline-localization | — | Missing boundary | Req 3.6 | domain | Req 3.6 | — | faultline-localization | P0 | faultline-localization | no |
| `missing_fail_boundary_yields_inconclusive` | No Fail observation yields Inconclusive | Only Pass recorded | faultline-localization | — | Missing boundary | Req 3.7 | domain | Req 3.7 | — | faultline-localization | P0 | faultline-localization | no |
| `non_monotonic_evidence_yields_low_confidence` | Fail before Pass yields low confidence | 4-commit seq, non-monotonic | faultline-localization | — | Confidence scoring | Req 3.5 | domain | Req 3.5 | — | faultline-localization | P0 | faultline-localization | no |
| `unobserved_between_boundaries_yields_inconclusive` | Unobserved commits between boundaries yields Inconclusive | 4-commit seq, gaps | faultline-localization | — | Incomplete evidence | — | domain | — | — | faultline-localization | P1 | faultline-localization | no |
| `first_bad_with_all_between_observed` | All observed, clean transition yields FirstBad | 3-commit seq, all observed | faultline-localization | — | Happy path | — | domain | Req 3.2 | — | faultline-localization | P1 | faultline-localization | no |
| `max_probes_exhausted_with_unobserved_between_boundaries` | Max probes + unobserved → Inconclusive with MaxProbesExhausted | max_probes=2, 5-commit seq | faultline-localization | — | Budget + incomplete | — | domain | Req 3.8 | — | faultline-localization | P1 | faultline-localization | no |
| `max_probes_exhausted_with_skipped_between_boundaries` | Max probes + Skip → SuspectWindow with MaxProbesExhausted | max_probes=3, 3-commit seq | faultline-localization | — | Budget + ambiguity | — | domain | Req 3.3, 3.8 | — | faultline-localization | P1 | faultline-localization | no |
| `max_probes_exhausted_missing_pass_boundary` | Max probes + no Pass → Inconclusive with both reasons | max_probes=1, only Fail | faultline-localization | — | Budget + missing boundary | — | domain | Req 3.6, 3.8 | — | faultline-localization | P1 | faultline-localization | no |
| `max_probes_not_exhausted_no_extra_reason` | Converged within budget → no MaxProbesExhausted | 2-commit seq, default policy | faultline-localization | — | Clean convergence | — | domain | Req 3.2 | — | faultline-localization | P2 | faultline-localization | no |
| `max_probes_exhausted_but_converged_yields_first_bad` | Converged exactly at budget → still FirstBad | max_probes=2, 2-commit seq | faultline-localization | — | Convergence priority | — | domain | Req 3.2, 3.8 | — | faultline-localization | P1 | faultline-localization | no |
| `prop_adjacent_pass_fail_yields_first_bad` | Adjacent Pass-Fail always yields FirstBad with high confidence | `n in 2..=20`, `i_frac` | faultline-localization | — | P5: Adjacent boundary | Req 3.2 | domain | Req 3.2 | — | faultline-localization | P0 | faultline-localization | yes |
| `prop_binary_narrowing_selects_valid_midpoint` | next_probe() returns a valid unobserved midpoint | `n in 3..=50` | faultline-localization | — | P4: Valid midpoint | Req 3.1 | domain | Req 3.1 | — | faultline-localization | P0 | faultline-localization | yes |
| `prop_first_bad_requires_direct_evidence` | FirstBad outcome has direct Pass and Fail observations at boundary | `n in 2..=20`, `i_frac` | faultline-localization | — | P10: Direct evidence | Req 3.9, 11.1 | domain | Req 3.9, 11.1 | — | faultline-localization | P0 | faultline-localization | yes |
| `prop_non_monotonic_evidence_yields_low_confidence` | Non-monotonic evidence always yields low confidence | `n in 4..=20`, fill selectors | faultline-localization | — | P7: Low confidence | Req 3.5 | domain | Req 3.5 | — | faultline-localization | P0 | faultline-localization | yes |
| `prop_only_pass_yields_inconclusive_missing_fail_boundary` | All-Pass observations yield Inconclusive with MissingFailBoundary | `n in 3..=15`, random indices | faultline-localization | — | P8: Missing boundary | Req 3.6, 3.7 | domain | Req 3.6, 3.7 | — | faultline-localization | P0 | faultline-localization | yes |
| `prop_only_fail_yields_inconclusive_missing_pass_boundary` | All-Fail observations yield Inconclusive with MissingPassBoundary | `n in 3..=15`, random indices | faultline-localization | — | P8: Missing boundary | Req 3.6, 3.7 | domain | Req 3.6, 3.7 | — | faultline-localization | P0 | faultline-localization | yes |
| `prop_monotonic_window_narrowing` | Window never expands during monotonic binary narrowing | `n in 3..=20`, transition_frac | faultline-localization | — | P21: Monotonic narrowing | Req 11.2 | domain | Req 11.2 | — | faultline-localization | P0 | faultline-localization | yes |
| `prop_suspect_window_confidence_cap` | SuspectWindow confidence is always < high | `n in 4..=20`, ambig selectors | faultline-localization | — | P22: Confidence cap | Req 11.3 | domain | Req 11.3 | — | faultline-localization | P0 | faultline-localization | yes |
| `prop_ambiguous_observations_yield_suspect_window` | Skip/Indeterminate between boundaries yields SuspectWindow with correct reasons | `n in 4..=20`, class selectors | faultline-localization | — | P6: Ambiguous observations | Req 3.3, 3.4 | domain | Req 3.3, 3.4 | — | faultline-localization | P0 | faultline-localization | yes |
| `prop_observation_order_independence` | Outcome is independent of observation recording order | `n in 3..=12`, permutation seeds | faultline-localization | — | P23: Order independence | Req 11.4 | domain | Req 11.4 | — | faultline-localization | P0 | faultline-localization | yes |
| `prop_max_probe_exhaustion_produces_explicit_outcome` | Max probes exhausted always includes MaxProbesExhausted reason | `n in 5..=30`, `max_probes in 2..=5` | faultline-localization | — | P25: Explicit exhaustion | Req 3.2 | domain | Req 3.2 | — | faultline-localization | P0 | faultline-localization | yes |
| `prop_observation_sequence_order_preservation` | observation_list() returns observations sorted by sequence_index | `n in 2..=20`, seq_indices | faultline-localization | — | P26: Order preservation | Req 3.3 | domain | Req 3.3 | — | faultline-localization | P0 | faultline-localization | yes |
| `prop_flaky_observations_degrade_confidence` | Flaky observations produce strictly lower confidence than stable | `n in 3..=15`, metamorphic | faultline-localization | — | P49: Flaky confidence degradation | Req 3.4 | domain | Req 3.4 | — | faultline-localization | P0 | faultline-localization | yes |
| `prop_default_flake_policy_no_signal` | Default FlakePolicy (retries=0) produces no FlakeSignal | `n in 2..=15`, class selectors | faultline-localization | — | P50: Default no signal | Req 3.6 | domain | Req 3.6 | — | faultline-localization | P0 | faultline-localization | yes |

#### faultline-localization — Fixture Scenarios

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `fixture_skipped_midpoint_yields_suspect_window` | Skipped midpoint produces SuspectWindow with SkippedRevision | `RevisionSequenceBuilder::with_labels` | faultline-localization, faultline-fixtures | — | Fixture: skipped midpoint | Req 7.5 | domain | Req 7.5 | — | faultline-localization | P1 | faultline-localization | no |
| `fixture_timed_out_midpoint_yields_suspect_window` | Timed-out midpoint produces SuspectWindow with IndeterminateRevision | `RevisionSequenceBuilder::with_labels` | faultline-localization, faultline-fixtures | — | Fixture: timed-out midpoint | Req 7.5 | domain | Req 7.5 | — | faultline-localization | P1 | faultline-localization | no |
| `fixture_non_monotonic_yields_low_confidence` | Non-monotonic evidence produces low confidence SuspectWindow | `RevisionSequenceBuilder::with_labels` | faultline-localization, faultline-fixtures | — | Fixture: non-monotonic | Req 7.5 | domain | Req 7.5 | — | faultline-localization | P1 | faultline-localization | no |

### faultline-surface

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `bucket_name_extracts_top_level_directory` | Top-level directory extraction | Fixed paths | faultline-surface | — | Bucketing correctness | — | domain | Req 5.2 | — | faultline-surface | P1 | faultline-surface | no |
| `bucket_name_root_level_file_uses_filename` | Root-level files use filename as bucket | Fixed paths | faultline-surface | — | Bucketing correctness | — | domain | Req 5.2 | — | faultline-surface | P1 | faultline-surface | no |
| `bucket_name_empty_path_returns_repo_root` | Empty path maps to "repo-root" | Empty/slash paths | faultline-surface | — | Edge case | — | domain | — | — | faultline-surface | P2 | faultline-surface | no |
| `surface_kind_source` | Source files classified as "source" | Fixed paths | faultline-surface | — | Classification | Req 5.2 | domain | Req 5.2 | — | faultline-surface | P1 | faultline-surface | no |
| `surface_kind_tests` | Test files classified as "tests" | Fixed paths | faultline-surface | — | Classification | Req 5.2 | domain | Req 5.2 | — | faultline-surface | P1 | faultline-surface | no |
| `surface_kind_benchmarks` | Benchmark files classified as "benchmarks" | Fixed paths | faultline-surface | — | Classification | — | domain | — | — | faultline-surface | P2 | faultline-surface | no |
| `surface_kind_scripts` | Script files classified as "scripts" | Fixed paths | faultline-surface | — | Classification | — | domain | — | — | faultline-surface | P2 | faultline-surface | no |
| `surface_kind_workflows` | Workflow files classified as "workflows" | Fixed paths | faultline-surface | — | Classification | — | domain | — | — | faultline-surface | P2 | faultline-surface | no |
| `surface_kind_docs` | Doc files classified as "docs" | Fixed paths | faultline-surface | — | Classification | — | domain | — | — | faultline-surface | P2 | faultline-surface | no |
| `surface_kind_build_script` | build.rs classified as "build-script" | Fixed paths | faultline-surface | — | Classification | — | domain | — | — | faultline-surface | P2 | faultline-surface | no |
| `surface_kind_lockfile` | Lock files classified as "lockfile" | Fixed paths | faultline-surface | — | Classification | — | domain | — | — | faultline-surface | P2 | faultline-surface | no |
| `surface_kind_migrations` | Migration files classified as "migrations" | Fixed paths | faultline-surface | — | Classification | — | domain | — | — | faultline-surface | P2 | faultline-surface | no |
| `surface_kind_other` | Unrecognized files classified as "other" | Fixed paths | faultline-surface | — | Classification | — | domain | — | — | faultline-surface | P2 | faultline-surface | no |
| `execution_surface_workflows` | Workflow files are execution surfaces | Fixed paths | faultline-surface | — | Execution surface | — | domain | — | — | faultline-surface | P1 | faultline-surface | no |
| `execution_surface_build_scripts` | build.rs files are execution surfaces | Fixed paths | faultline-surface | — | Execution surface | — | domain | — | — | faultline-surface | P1 | faultline-surface | no |
| `execution_surface_shell_scripts` | Shell scripts are execution surfaces | Fixed paths | faultline-surface | — | Execution surface | — | domain | — | — | faultline-surface | P1 | faultline-surface | no |
| `non_execution_surface` | Source/config files are not execution surfaces | Fixed paths | faultline-surface | — | Negative case | — | domain | — | — | faultline-surface | P1 | faultline-surface | no |
| `summarize_empty_input` | Empty input produces empty summary | Empty slice | faultline-surface | — | Edge case | — | domain | — | — | faultline-surface | P2 | faultline-surface | no |
| `summarize_groups_by_top_level_directory` | Changes grouped by top-level directory | Mixed paths | faultline-surface | — | Grouping correctness | Req 5.3 | domain | Req 5.3 | — | faultline-surface | P0 | faultline-surface | no |
| `summarize_collects_execution_surfaces` | Execution surfaces collected separately | Mixed paths | faultline-surface | — | Separation | Req 5.4 | domain | Req 5.4 | — | faultline-surface | P0 | faultline-surface | no |
| `summarize_assigns_multiple_surface_kinds_per_bucket` | Buckets can have multiple surface kinds | Mixed paths in one dir | faultline-surface | — | Multi-kind buckets | — | domain | — | — | faultline-surface | P2 | faultline-surface | no |
| `summarize_total_changes_equals_input_length` | total_changes matches input count | 5 mixed paths | faultline-surface | — | Count correctness | — | domain | — | — | faultline-surface | P1 | faultline-surface | no |
| `summarize_every_path_in_exactly_one_bucket` | Every path appears in exactly one bucket | 4 mixed paths | faultline-surface | — | Partition correctness | — | domain | — | — | faultline-surface | P1 | faultline-surface | no |
| `prop_surface_analysis_invariants` | Surface analysis maintains all structural invariants | `arb_path_change()` × 1..30 | faultline-surface | — | P13: Surface invariants | Req 5.2, 5.3, 5.4 | domain | Req 5.2, 5.3, 5.4 | — | faultline-surface | P0 | faultline-surface | yes |
| `rank_suspect_surface_empty_input` | Empty input produces empty ranking | Empty slice | faultline-surface | — | Edge case | — | domain | Req 1.1 | — | faultline-surface | P2 | faultline-surface | no |
| `rank_suspect_surface_single_modified_source` | Single modified source scored correctly | Single PathChange | faultline-surface | — | Scoring correctness | — | domain | Req 1.1, 1.2 | — | faultline-surface | P1 | faultline-surface | no |
| `rank_suspect_surface_scoring_rules` | Scoring rules applied correctly for mixed changes | Mixed PathChange set | faultline-surface | — | Scoring correctness | — | domain | Req 1.1, 1.2 | — | faultline-surface | P0 | faultline-surface | no |
| `rank_suspect_surface_owner_hints` | Owner hints populated from owners map | PathChanges + owners map | faultline-surface | — | Owner hint fidelity | — | domain | Req 1.3, 1.4, 1.5 | — | faultline-surface | P1 | faultline-surface | no |
| `rank_suspect_surface_execution_surface_deleted` | Execution surface + deleted file scored highest | Mixed PathChanges | faultline-surface | — | Score ordering | — | domain | Req 1.2 | — | faultline-surface | P0 | faultline-surface | no |
| `prop_suspect_ranking_sorted_and_deterministic` | Ranking is sorted by descending score, ascending path, and deterministic | `arb_path_change()` × 0..30 | faultline-surface | — | P43: Sorted & deterministic | Req 1.1, 1.10 | domain | Req 1.1, 1.10 | — | faultline-surface | P0 | faultline-surface | yes |
| `prop_exec_rename_delete_score_higher_than_ordinary` | Execution surfaces, renames, deletes score higher than ordinary | `arb_mixed_path_changes()` | faultline-surface | — | P44: Score ordering | Req 1.2 | domain | Req 1.2 | — | faultline-surface | P0 | faultline-surface | yes |
| `prop_suspect_entry_preserves_status_and_surface_kind` | SuspectEntry preserves change_status and surface_kind | `arb_path_change()` | faultline-surface | — | P45: Field consistency | Req 1.6, 1.7 | domain | Req 1.6, 1.7 | — | faultline-surface | P0 | faultline-surface | yes |
| `prop_suspect_entry_owner_hint_matches_map` | SuspectEntry owner_hint matches owners map | `arb_path_change()` × 1..20 + owners | faultline-surface | — | P46: Owner hint fidelity | Req 1.3, 1.4, 1.5 | domain | Req 1.3, 1.4, 1.5 | — | faultline-surface | P0 | faultline-surface | yes |

### faultline-fixtures

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `push_and_build_produces_correct_sequence` | Push + build produces correct RevisionSequence | `RevisionSequenceBuilder::new()` | faultline-fixtures | — | Builder correctness | — | domain | — | — | faultline-fixtures | P1 | faultline-fixtures | no |
| `exact_boundary_produces_n_commits` | exact_boundary(n) produces n-commit sequence | `RevisionSequenceBuilder::exact_boundary` | faultline-fixtures | — | Builder correctness | — | domain | — | — | faultline-fixtures | P1 | faultline-fixtures | no |
| `exact_boundary_zero_produces_empty_sequence` | exact_boundary(0) produces empty sequence | `RevisionSequenceBuilder::exact_boundary(0)` | faultline-fixtures | — | Edge case | — | domain | — | — | faultline-fixtures | P2 | faultline-fixtures | no |
| `with_labels_produces_correct_sequence` | with_labels produces sequence with given labels | `RevisionSequenceBuilder::with_labels` | faultline-fixtures | — | Builder correctness | — | domain | — | — | faultline-fixtures | P1 | faultline-fixtures | no |
| `with_labels_empty_produces_empty_sequence` | with_labels([]) produces empty sequence | `RevisionSequenceBuilder::with_labels(&[])` | faultline-fixtures | — | Edge case | — | domain | — | — | faultline-fixtures | P2 | faultline-fixtures | no |
| `build_with_fewer_than_two_commits_still_works` | Builder with <2 commits still produces valid sequence | `RevisionSequenceBuilder::new()` | faultline-fixtures | — | Edge case | — | domain | — | — | faultline-fixtures | P2 | faultline-fixtures | no |
| `git_repo_builder_creates_repo_with_commits` | GitRepoBuilder creates real Git repo with commits | `GitRepoBuilder::new()` | faultline-fixtures | — | Builder correctness | — | domain | — | — | faultline-fixtures | P1 | faultline-fixtures | no |
| `git_repo_builder_supports_delete` | GitRepoBuilder supports file deletion | `GitRepoBuilder + FileOp::Delete` | faultline-fixtures | — | Builder correctness | — | domain | — | — | faultline-fixtures | P1 | faultline-fixtures | no |
| `git_repo_builder_supports_rename` | GitRepoBuilder supports file rename | `GitRepoBuilder + FileOp::Rename` | faultline-fixtures | — | Builder correctness | — | domain | — | — | faultline-fixtures | P1 | faultline-fixtures | no |
| `git_repo_builder_merge` | GitRepoBuilder supports merge commits | `GitRepoBuilder::merge()` | faultline-fixtures | — | Builder correctness | — | domain | — | — | faultline-fixtures | P1 | faultline-fixtures | no |
| `git_repo_builder_subdirectories` | GitRepoBuilder supports nested directory paths | `GitRepoBuilder + nested paths` | faultline-fixtures | — | Builder correctness | — | domain | — | — | faultline-fixtures | P1 | faultline-fixtures | no |
| `prop_revision_sequence_boundary_invariant` | RevisionSequence boundary invariant holds for arbitrary SHAs | `arb good/bad SHA strings` | faultline-fixtures | — | Boundary invariant | — | domain | — | — | faultline-fixtures | P0 | faultline-fixtures | yes |

---

## Adapter Tier — BDD, Property, and Golden Tests

### faultline-probe-exec

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `prop_exit_code_classification` | Exit codes map to correct ObservationClass | `arb exit_code × timed_out` | faultline-probe-exec | — | P1: Exit code classification | Req 2.3–2.6 | adapter | Req 2.3–2.6 | — | faultline-probe-exec | P0 | faultline-probe-exec | yes |
| `prop_signal_aware_exit_code_classification` | Signal-aware classification handles all combinations | `arb exit_code × timed_out × signal_number` | faultline-probe-exec | — | P27: Signal-aware classification | Req 5.1, 5.2 | adapter | Req 5.1, 5.2 | — | faultline-probe-exec | P0 | faultline-probe-exec | yes |
| `prop_observation_structural_completeness` | Probe observations contain all required fields | `arb_commit_id() × arb_probe_kind()` | faultline-probe-exec | — | P2/P28: Structural completeness | Req 2.7, 4.7, 5.4 | adapter | Req 2.7, 4.7, 5.4 | — | faultline-probe-exec | P0 | faultline-probe-exec | yes |
| `truncate_output_under_limit_unchanged` | Output under limit is unchanged | Fixed short string | faultline-probe-exec | — | Truncation correctness | — | adapter | — | — | faultline-probe-exec | P2 | faultline-probe-exec | no |
| `truncate_output_at_limit_unchanged` | Output exactly at limit is unchanged | String of limit length | faultline-probe-exec | — | Truncation boundary | — | adapter | — | — | faultline-probe-exec | P2 | faultline-probe-exec | no |
| `truncate_output_over_limit_truncated` | Output over limit is truncated with marker | String exceeding limit | faultline-probe-exec | — | Truncation correctness | — | adapter | — | — | faultline-probe-exec | P1 | faultline-probe-exec | no |
| `truncate_output_empty_string` | Empty string is unchanged | Empty string | faultline-probe-exec | — | Edge case | — | adapter | — | — | faultline-probe-exec | P2 | faultline-probe-exec | no |
| `signal_termination_sets_signal_number` | Signal-killed process sets signal_number field | Shell probe with self-SIGTERM | faultline-probe-exec | — | Signal detection | Req 5.1 | adapter | Req 5.1 | — | faultline-probe-exec | P0 | faultline-probe-exec | no |
| `probe_output_exceeding_limit_is_truncated` | Real probe output exceeding limit is truncated | Shell probe with large output | faultline-probe-exec | — | Integration truncation | — | adapter | — | — | faultline-probe-exec | P1 | faultline-probe-exec | no |

### faultline-store

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `prepare_run_creates_directory_and_request_json` | Run preparation creates directory and persists request | Sample request + TempDir | faultline-store | request.json | Persistence | — | adapter | — | request.json | faultline-store | P1 | faultline-store | no |
| `prepare_run_sets_resumed_on_second_call` | Second prepare_run sets resumed=true | Same request twice | faultline-store | — | Resume detection | — | adapter | — | — | faultline-store | P1 | faultline-store | no |
| `load_observations_returns_empty_when_no_file` | Missing observations file returns empty vec | Fresh run | faultline-store | — | Default behavior | — | adapter | — | — | faultline-store | P2 | faultline-store | no |
| `save_and_load_single_observation` | Single observation round-trips through store | Sample observation + TempDir | faultline-store | observations.json | Round-trip | — | adapter | — | observations.json | faultline-store | P0 | faultline-store | no |
| `save_observation_upserts_by_commit_id` | Saving same commit replaces previous observation | Two observations, same commit | faultline-store | observations.json | Upsert semantics | — | adapter | — | observations.json | faultline-store | P1 | faultline-store | no |
| `save_observation_sorts_by_sequence_index` | Observations sorted by sequence_index on disk | Out-of-order observations | faultline-store | observations.json | Sort order | — | adapter | — | observations.json | faultline-store | P1 | faultline-store | no |
| `save_and_read_report` | Report round-trips through store | Sample report + TempDir | faultline-store | report.json | Round-trip | — | adapter | — | report.json | faultline-store | P0 | faultline-store | no |
| `load_report_returns_none_when_no_file` | Missing report file returns None | Fresh run | faultline-store | — | Default behavior | — | adapter | — | — | faultline-store | P2 | faultline-store | no |
| `load_report_returns_saved_report` | Saved report can be loaded back | Sample report + TempDir | faultline-store | report.json | Round-trip | — | adapter | — | report.json | faultline-store | P0 | faultline-store | no |
| `new_creates_root_directory` | FileRunStore::new creates nested directories | Deep nested path | faultline-store | — | Directory creation | — | adapter | — | — | faultline-store | P2 | faultline-store | no |
| `prepare_run_creates_lock_file` | Lock file created with PID and timestamp | Sample request + TempDir | faultline-store | .lock | Lock creation | — | adapter | — | .lock | faultline-store | P1 | faultline-store | no |
| `prepare_run_allows_same_process_reentry` | Same process can re-acquire lock | Same request twice | faultline-store | .lock | Re-entry | — | adapter | — | .lock | faultline-store | P1 | faultline-store | no |
| `prepare_run_rejects_lock_held_by_another_live_process` | Lock held by live process is rejected (Unix) | Lock with PID 1 | faultline-store | — | Lock enforcement | — | adapter | — | — | faultline-store | P0 | faultline-store | no |
| `prepare_run_cleans_stale_lock_from_dead_process` | Stale lock from dead process is cleaned | Lock with high PID | faultline-store | .lock | Stale lock cleanup | — | adapter | — | .lock | faultline-store | P1 | faultline-store | no |
| `save_report_releases_lock` | Lock released after saving report | Full run lifecycle | faultline-store | — | Lock release | — | adapter | — | — | faultline-store | P1 | faultline-store | no |
| `atomic_write_produces_correct_content_and_no_tmp_file` | Atomic write produces correct content, no .tmp residue | Fixed content + TempDir | faultline-store | — | Atomic write | — | adapter | — | — | faultline-store | P1 | faultline-store | no |
| `save_probe_logs_creates_log_files` | Probe logs saved to per-commit files | Sample logs + TempDir | faultline-store | logs/*.log | Log persistence | — | adapter | — | logs/*.log | faultline-store | P1 | faultline-store | no |
| `save_probe_logs_creates_logs_directory` | Logs directory created on first save | Fresh run | faultline-store | logs/ | Directory creation | — | adapter | — | logs/ | faultline-store | P2 | faultline-store | no |
| `save_probe_logs_overwrites_existing_logs` | Re-saving logs overwrites previous content | Two saves, same commit | faultline-store | logs/*.log | Overwrite semantics | — | adapter | — | logs/*.log | faultline-store | P1 | faultline-store | no |
| `save_probe_logs_handles_multiple_commits` | Multiple commits have separate log files | Two different commits | faultline-store | logs/*.log | Isolation | — | adapter | — | logs/*.log | faultline-store | P1 | faultline-store | no |
| `prop_observation_round_trip` | Observation round-trips through store | `arb_probe_observation()` | faultline-store | observations.json | P11: Round-trip | — | adapter | — | observations.json | faultline-store | P0 | faultline-store | yes |
| `prop_report_round_trip` | Report round-trips through store | `arb_analysis_report()` | faultline-store | report.json | P12: Round-trip | — | adapter | — | report.json | faultline-store | P0 | faultline-store | yes |
| `prop_request_round_trip` | Request round-trips through store | `arb_analysis_request()` | faultline-store | request.json | Round-trip | — | adapter | — | request.json | faultline-store | P0 | faultline-store | yes |
| `prop_store_observation_sequence_order` | Observations stored in sequence_index order | `arb_probe_observation() × 2..10` | faultline-store | observations.json | P26: Sequence order | Req 3.3, 6.5 | adapter | Req 3.3, 6.5 | observations.json | faultline-store | P0 | faultline-store | yes |
| `prop_run_store_resumability` | Run store supports resume with cached observations | `arb_analysis_request() + observations` | faultline-store | observations.json | Resumability | Req 4.3 | adapter | Req 4.3 | observations.json | faultline-store | P0 | faultline-store | yes |
| `prop_report_load_round_trip` | Report load round-trips through store | `arb_analysis_report()` | faultline-store | report.json | Round-trip | Req 6.11 | adapter | Req 6.11 | report.json | faultline-store | P0 | faultline-store | yes |
| `prop_schema_version_round_trip` | Schema version preserved through store round-trip | `arb_analysis_report() + custom version` | faultline-store | report.json | Version preservation | Req 1.4, 1.6 | adapter | Req 1.4, 1.6 | report.json | faultline-store | P0 | faultline-store | yes |
| `prop_version_metadata_persistence` | Version metadata persisted correctly | `arb_analysis_request()` | faultline-store | request.json | Metadata persistence | Req 6.4 | adapter | Req 6.4 | request.json | faultline-store | P0 | faultline-store | yes |

### faultline-git

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `codeowners_parse_valid_file` | Valid CODEOWNERS file parsed correctly | Hand-built content | faultline-git | — | Parse correctness | — | adapter | Req 1.3 | — | faultline-git | P1 | faultline-git | no |
| `codeowners_parse_empty_file` | Empty CODEOWNERS returns empty rules | Empty string | faultline-git | — | Edge case | — | adapter | Req 1.3 | — | faultline-git | P2 | faultline-git | no |
| `codeowners_parse_comments_only` | Comments-only CODEOWNERS returns empty rules | Comment lines | faultline-git | — | Edge case | — | adapter | Req 1.3 | — | faultline-git | P2 | faultline-git | no |
| `codeowners_parse_malformed_line_skipped` | Malformed lines skipped gracefully | Mixed valid/invalid lines | faultline-git | — | Graceful degradation | — | adapter | Req 1.3 | — | faultline-git | P1 | faultline-git | no |
| `codeowners_last_match_wins` | Last matching pattern wins | Overlapping patterns | faultline-git | — | Match precedence | — | adapter | Req 1.3 | — | faultline-git | P0 | faultline-git | no |
| `codeowners_wildcard_matches_all` | Wildcard pattern matches all paths | `* @owner` pattern | faultline-git | — | Wildcard matching | — | adapter | Req 1.3 | — | faultline-git | P1 | faultline-git | no |
| `codeowners_directory_pattern` | Directory pattern matches paths under directory | `/docs/` pattern | faultline-git | — | Directory matching | — | adapter | Req 1.3 | — | faultline-git | P1 | faultline-git | no |
| `codeowners_doublestar_pattern` | Double-star pattern matches recursively | `**/*.py` pattern | faultline-git | — | Recursive matching | — | adapter | Req 1.3 | — | faultline-git | P1 | faultline-git | no |
| `codeowners_no_match_returns_none` | Unmatched path returns None | Non-matching path | faultline-git | — | No-match behavior | — | adapter | Req 1.3, 1.5 | — | faultline-git | P1 | faultline-git | no |
| `codeowners_from_github_dir` | CODEOWNERS read from .github/ directory | GitRepoBuilder + .github/CODEOWNERS | faultline-git | — | File discovery | — | adapter | Req 1.3 | — | faultline-git | P1 | faultline-git | no |
| `codeowners_from_repo_root` | CODEOWNERS read from repo root | GitRepoBuilder + CODEOWNERS | faultline-git | — | File discovery | — | adapter | Req 1.3 | — | faultline-git | P1 | faultline-git | no |
| `codeowners_missing_returns_none_owners` | Missing CODEOWNERS returns None for all paths | GitRepoBuilder without CODEOWNERS | faultline-git | — | Graceful absence | — | adapter | Req 1.3, 1.5 | — | faultline-git | P1 | faultline-git | no |
| `blame_frequency_returns_most_frequent_author` | Most frequent committer returned as owner | GitRepoBuilder with multiple commits | faultline-git | — | Frequency heuristic | — | adapter | Req 1.4 | — | faultline-git | P1 | faultline-git | no |
| `blame_frequency_no_commits_returns_none` | No commits for path returns None | GitRepoBuilder with empty history | faultline-git | — | Edge case | — | adapter | Req 1.4, 1.5 | — | faultline-git | P2 | faultline-git | no |
| `prop_codeowners_parser_determinism` | CODEOWNERS parsing is deterministic for any content and path | Random CODEOWNERS content + paths | faultline-git | — | P55: Parser determinism | Req 1.3 | adapter | Req 1.3 | — | faultline-git | P0 | faultline-git | yes |
| `changed_paths_detects_add_modify_delete_rename` | Git diff parsing detects add, modify, delete, and rename | GitRepoBuilder with mixed ops | faultline-git | — | Diff parsing correctness | — | adapter | — | — | faultline-git | P0 | faultline-git | no |
| `changed_paths_empty_diff_returns_empty_vec` | Empty diff returns empty changed paths | GitRepoBuilder with no changes | faultline-git | — | Edge case | — | adapter | — | — | faultline-git | P2 | faultline-git | no |
| `cleans_stale_worktrees_on_construction` | GitAdapter cleans stale worktrees on construction | GitRepoBuilder + stale worktree | faultline-git | — | Cleanup correctness | — | adapter | — | — | faultline-git | P1 | faultline-git | no |
| `cleanup_checkout_returns_ok_on_missing_directory` | cleanup_checkout returns Ok for missing directory | Non-existent path | faultline-git | — | Graceful degradation | — | adapter | — | — | faultline-git | P2 | faultline-git | no |
| `exact_first_bad_commit_real_git` | End-to-end localization finds exact first bad commit | GitRepoBuilder + real git | faultline-git | — | Integration correctness | — | adapter | — | — | faultline-git | P0 | faultline-git | no |
| `first_parent_merge_history_real_git` | First-parent merge history linearized correctly | GitRepoBuilder with merges | faultline-git | — | Linearization correctness | — | adapter | — | — | faultline-git | P0 | faultline-git | no |
| `invalid_boundaries_real_git` | Invalid boundaries rejected by real git adapter | GitRepoBuilder + invalid boundaries | faultline-git | — | Boundary validation | — | adapter | — | — | faultline-git | P0 | faultline-git | no |
| `rejects_non_repo_path` | GitAdapter rejects non-repository path | Non-repo temp directory | faultline-git | — | Input validation | — | adapter | — | — | faultline-git | P1 | faultline-git | no |
| `rename_and_delete_real_git` | Rename and delete detected in real git repo | GitRepoBuilder with rename/delete | faultline-git | — | Diff parsing correctness | — | adapter | — | — | faultline-git | P0 | faultline-git | no |
| `prop_worktree_path_uniqueness` | Worktree paths are unique for different commit SHAs | `arb SHA strings` | faultline-git | — | Path uniqueness | — | adapter | — | — | faultline-git | P0 | faultline-git | yes |

### faultline-render

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `render_writes_analysis_json` | Render produces analysis.json | Sample report + TempDir | faultline-render | analysis.json | File creation | Req 6.1 | adapter | Req 6.1 | analysis.json | faultline-render | P0 | faultline-render | no |
| `analysis_json_contains_all_fields` | analysis.json has all required top-level fields | Sample report | faultline-render | analysis.json | Field completeness | Req 6.2 | adapter | Req 6.2 | analysis.json | faultline-render | P0 | faultline-render | no |
| `analysis_json_is_deterministic` | Same report produces identical JSON | Sample report × 2 dirs | faultline-render | analysis.json | Determinism | Req 6.3 | adapter | Req 6.3 | analysis.json | faultline-render | P0 | faultline-render | no |
| `analysis_json_is_valid_json` | analysis.json is valid JSON | Sample report | faultline-render | analysis.json | Validity | Req 6.4 | adapter | Req 6.4 | analysis.json | faultline-render | P1 | faultline-render | no |
| `render_creates_output_directory` | Render creates output directory if missing | Non-existent dir | faultline-render | — | Directory creation | — | adapter | — | — | faultline-render | P2 | faultline-render | no |
| `output_dir_returns_configured_path` | output_dir() accessor works | Fixed path | faultline-render | — | Accessor | — | adapter | — | — | faultline-render | P2 | faultline-render | no |
| `analysis_json_round_trips` | JSON round-trip preserves report equality | Sample report | faultline-render | analysis.json | Round-trip | — | adapter | — | analysis.json | faultline-render | P0 | faultline-render | no |
| `render_writes_index_html` | Render produces index.html | Sample report + TempDir | faultline-render | index.html | File creation | Req 7.1 | adapter | Req 7.1 | index.html | faultline-render | P0 | faultline-render | no |
| `html_contains_run_id` | HTML contains the run ID | Sample report | faultline-render | index.html | Content | Req 7.2 | adapter | Req 7.2 | index.html | faultline-render | P1 | faultline-render | no |
| `html_contains_first_bad_outcome` | HTML shows FirstBad outcome details | FirstBad report | faultline-render | index.html | Content | Req 7.2 | adapter | Req 7.2 | index.html | faultline-render | P1 | faultline-render | no |
| `html_contains_suspect_window_outcome` | HTML shows SuspectWindow outcome details | SuspectWindow report | faultline-render | index.html | Content | Req 7.2 | adapter | Req 7.2 | index.html | faultline-render | P1 | faultline-render | no |
| `html_contains_inconclusive_outcome` | HTML shows Inconclusive outcome | Inconclusive report | faultline-render | index.html | Content | Req 7.2 | adapter | Req 7.2 | index.html | faultline-render | P1 | faultline-render | no |
| `html_contains_probe_fingerprint_and_history_mode` | HTML contains probe fingerprint and history mode | Sample report | faultline-render | index.html | Content | Req 7.2 | adapter | Req 7.2 | index.html | faultline-render | P1 | faultline-render | no |
| `html_contains_observation_timeline_rows` | HTML has correct number of observation rows | Sample report (2 obs) | faultline-render | index.html | Row count | Req 7.2 | adapter | Req 7.2 | index.html | faultline-render | P1 | faultline-render | no |
| `html_contains_surface_buckets` | HTML contains surface bucket information | Sample report | faultline-render | index.html | Content | Req 7.2 | adapter | Req 7.2 | index.html | faultline-render | P1 | faultline-render | no |
| `html_contains_changed_paths` | HTML contains changed path information | Sample report | faultline-render | index.html | Content | Req 7.2 | adapter | Req 7.2 | index.html | faultline-render | P1 | faultline-render | no |
| `html_has_no_external_dependencies` | HTML has no external links, scripts, or images | Sample report | faultline-render | index.html | Self-contained | Req 7.3 | adapter | Req 7.3 | index.html | faultline-render | P0 | faultline-render | no |
| `html_has_inline_css` | HTML has inline `<style>` block | Sample report | faultline-render | index.html | Self-contained | Req 7.3 | adapter | Req 7.3 | index.html | faultline-render | P1 | faultline-render | no |
| `escape_html_replaces_special_chars` | HTML escaping handles all special characters | XSS-like input | faultline-render | — | Security | Req 7.5 | adapter | Req 7.5 | — | faultline-render | P0 | faultline-render | no |
| `html_escapes_dynamic_content` | Dynamic content in HTML is escaped | Report with `<script>` in run_id | faultline-render | index.html | Security | Req 7.5 | adapter | Req 7.5 | index.html | faultline-render | P0 | faultline-render | no |
| `html_has_valid_structure` | HTML has doctype, html, head, body | Sample report | faultline-render | index.html | Structure | Req 7.1 | adapter | Req 7.1 | index.html | faultline-render | P1 | faultline-render | no |
| `html_has_outcome_firstbad_class` | FirstBad uses outcome-firstbad CSS class | FirstBad report | faultline-render | index.html | Visual distinction | Req 8.8 | adapter | Req 8.8 | index.html | faultline-render | P1 | faultline-render | no |
| `html_has_outcome_suspect_class` | SuspectWindow uses outcome-suspect CSS class | SuspectWindow report | faultline-render | index.html | Visual distinction | Req 8.8 | adapter | Req 8.8 | index.html | faultline-render | P1 | faultline-render | no |
| `html_has_outcome_inconclusive_class` | Inconclusive uses outcome-inconclusive CSS class | Inconclusive report | faultline-render | index.html | Visual distinction | Req 8.8 | adapter | Req 8.8 | index.html | faultline-render | P1 | faultline-render | no |
| `html_renders_ambiguity_badges_suspect_window` | SuspectWindow renders reason badges | SuspectWindow with reasons | faultline-render | index.html | Badges | Req 8.9 | adapter | Req 8.9 | index.html | faultline-render | P1 | faultline-render | no |
| `html_renders_ambiguity_badges_inconclusive` | Inconclusive renders reason badges | Inconclusive with reasons | faultline-render | index.html | Badges | Req 8.9 | adapter | Req 8.9 | index.html | faultline-render | P1 | faultline-render | no |
| `html_observations_sorted_by_sequence_index` | Observations sorted by sequence_index in HTML | Report with reversed indices | faultline-render | index.html | Sort order | Req 8.10 | adapter | Req 8.10 | index.html | faultline-render | P1 | faultline-render | no |
| `golden_analysis_json` | Golden snapshot of canonical analysis.json | `canonical_fixture_report()` | faultline-render | analysis.json snapshot | Golden contract | Req 3.4 | adapter | Req 3.4 | analysis.json | faultline-render | P0 | faultline-render | yes |
| `golden_index_html` | Golden snapshot of canonical index.html | `canonical_fixture_report()` | faultline-render | index.html snapshot | Golden contract | Req 3.4 | adapter | Req 3.4 | index.html | faultline-render | P0 | faultline-render | yes |
| `prop_html_contains_required_data` | HTML contains all required data from report | `arb_analysis_report()` | faultline-render | index.html | P16: Required data | Req 7.2 | adapter | Req 7.2 | index.html | faultline-render | P0 | faultline-render | yes |
| `prop_html_escaping_correctness` | HTML escaping prevents injection | Arbitrary strings with special chars | faultline-render | — | P17: Escaping | Req 7.5 | adapter | Req 7.5 | — | faultline-render | P0 | faultline-render | yes |
| `prop_html_is_self_contained` | HTML has no external dependencies | `arb_analysis_report()` | faultline-render | index.html | P18: Self-contained | Req 7.3 | adapter | Req 7.3 | index.html | faultline-render | P0 | faultline-render | yes |
| `prop_html_outcome_visual_distinction_and_badges` | Outcome CSS classes and badges are correct | `arb_analysis_report()` | faultline-render | index.html | P29: Visual distinction | Req 8.8, 8.9 | adapter | Req 8.8, 8.9 | index.html | faultline-render | P0 | faultline-render | yes |
| `prop_html_temporal_observation_order` | Observations in HTML sorted by sequence_index | `arb_analysis_report()` (≥2 obs) | faultline-render | index.html | P30: Temporal order | Req 8.10 | adapter | Req 8.10 | index.html | faultline-render | P0 | faultline-render | yes |
| `prop_html_execution_surface_separation` | Execution surfaces rendered separately | `arb_analysis_report()` (non-empty exec) | faultline-render | index.html | P31: Surface separation | Req 8.11 | adapter | Req 8.11 | index.html | faultline-render | P0 | faultline-render | yes |
| `prop_markdown_dossier_contains_required_sections` | Markdown dossier contains all required sections | `arb_analysis_report()` (non-Inconclusive) | faultline-render | dossier.md | P47: Section completeness | Req 2.1–2.5 | adapter | Req 2.1–2.5 | dossier.md | faultline-render | P0 | faultline-render | yes |
| `html_observation_rows_have_color_classes` | Observation rows have correct CSS color classes | Sample report | faultline-render | index.html | Visual distinction | Req 8.10 | adapter | Req 8.10 | index.html | faultline-render | P1 | faultline-render | no |
| `html_indeterminate_row_shows_signal_badge` | Indeterminate row with signal_number shows signal badge | Sample report + signal_number | faultline-render | index.html | Signal display | Req 8.10 | adapter | Req 8.10 | index.html | faultline-render | P1 | faultline-render | no |
| `html_renders_execution_surfaces_section` | Execution surfaces rendered in separate HTML section | Sample report + execution surfaces | faultline-render | index.html | Surface separation | Req 8.11 | adapter | Req 8.11 | index.html | faultline-render | P1 | faultline-render | no |
| `html_no_execution_surfaces_when_empty` | No execution surfaces section when list is empty | Sample report (empty exec surfaces) | faultline-render | index.html | Conditional rendering | Req 8.11 | adapter | Req 8.11 | index.html | faultline-render | P1 | faultline-render | no |
| `html_renders_log_links_for_truncated_output` | Truncated stdout renders log file link | Sample report + truncated output | faultline-render | index.html | Log link rendering | Req 8.12 | adapter | Req 8.12 | index.html | faultline-render | P1 | faultline-render | no |
| `html_renders_log_links_for_truncated_stderr` | Truncated stderr renders log file link | Sample report + truncated stderr | faultline-render | index.html | Log link rendering | Req 8.12 | adapter | Req 8.12 | index.html | faultline-render | P1 | faultline-render | no |
| `html_no_log_section_when_no_truncation` | No log section when no truncated output | Sample report (no truncation) | faultline-render | index.html | Conditional rendering | Req 8.12 | adapter | Req 8.12 | index.html | faultline-render | P1 | faultline-render | no |
| `snapshot_analysis_json_structure` | Snapshot test for canonical analysis.json structure | `canonical_fixture_report()` | faultline-render | analysis.json | Structural correctness | Req 6.1, 6.2 | adapter | Req 6.1, 6.2 | analysis.json | faultline-render | P0 | faultline-render | yes |
| `snapshot_html_report_structure` | Snapshot test for canonical HTML report structure | `canonical_fixture_report()` | faultline-render | index.html | Structural correctness | Req 7.1, 7.2 | adapter | Req 7.1, 7.2 | index.html | faultline-render | P0 | faultline-render | yes |

### faultline-sarif

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `sarif_first_bad_produces_error_level` | FirstBad maps to SARIF error level | FirstBad report | faultline-sarif | SARIF JSON | Level mapping | Req 3.6 | adapter | Req 3.6 | SARIF JSON | faultline-sarif | P0 | faultline-sarif | no |
| `sarif_suspect_window_produces_warning_level` | SuspectWindow maps to SARIF warning level | SuspectWindow report | faultline-sarif | SARIF JSON | Level mapping | Req 3.6 | adapter | Req 3.6 | SARIF JSON | faultline-sarif | P0 | faultline-sarif | no |
| `sarif_inconclusive_produces_note_level` | Inconclusive maps to SARIF note level | Inconclusive report | faultline-sarif | SARIF JSON | Level mapping | Req 3.6 | adapter | Req 3.6 | SARIF JSON | faultline-sarif | P0 | faultline-sarif | no |
| `sarif_empty_changed_paths_produces_empty_locations` | Empty changed_paths produces empty locations | Report with no paths | faultline-sarif | SARIF JSON | Edge case | Req 3.6 | adapter | Req 3.6 | SARIF JSON | faultline-sarif | P2 | faultline-sarif | no |
| `prop_sarif_export_structural_validity` | SARIF output is structurally valid for all reports | `arb_analysis_report()` | faultline-sarif | SARIF JSON | P41: Structural validity | Req 3.6 | adapter | Req 3.6 | SARIF JSON | faultline-sarif | P0 | faultline-sarif | yes |

### faultline-junit

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `junit_first_bad_has_failure_element` | FirstBad maps to JUnit failure element | FirstBad report | faultline-junit | JUnit XML | Failure mapping | — | adapter | — | JUnit XML | faultline-junit | P0 | faultline-junit | no |
| `junit_suspect_window_has_failure_element` | SuspectWindow maps to JUnit failure element | SuspectWindow report | faultline-junit | JUnit XML | Failure mapping | — | adapter | — | JUnit XML | faultline-junit | P0 | faultline-junit | no |
| `junit_inconclusive_has_failure_element` | Inconclusive maps to JUnit failure element | Inconclusive report | faultline-junit | JUnit XML | Failure mapping | — | adapter | — | JUnit XML | faultline-junit | P0 | faultline-junit | no |
| `junit_observations_in_system_out` | Observations rendered in system-out element | FirstBad report with observations | faultline-junit | JUnit XML | Content completeness | — | adapter | — | JUnit XML | faultline-junit | P1 | faultline-junit | no |
| `junit_empty_observations` | Empty observations produce valid JUnit XML | Report with no observations | faultline-junit | JUnit XML | Edge case | — | adapter | — | JUnit XML | faultline-junit | P2 | faultline-junit | no |
| `prop_junit_xml_export_structural_validity` | JUnit XML output is structurally valid for all reports | `arb_analysis_report()` | faultline-junit | JUnit XML | P42: Structural validity | — | adapter | — | JUnit XML | faultline-junit | P0 | faultline-junit | yes |

---

## App Tier — Integration and Orchestration Tests

### faultline-app

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `integration_cached_resume_skips_cached_commits` | Cached observations are reused on resume | Mock ports + 5-commit seq | faultline-app | — | Resume correctness | — | app | — | — | faultline-app | P0 | faultline-app | no |
| `integration_good_boundary_fail_yields_invalid_boundary` | Good boundary returning Fail yields InvalidBoundary error | Mock ports + 5-commit seq | faultline-app | — | Boundary validation | Req 10.1–10.5 | app | Req 10.1–10.5 | — | faultline-app | P0 | faultline-app | no |
| `integration_bad_boundary_pass_yields_invalid_boundary` | Bad boundary returning Pass yields InvalidBoundary error | Mock ports + 5-commit seq | faultline-app | — | Boundary validation | Req 10.1–10.5 | app | Req 10.1–10.5 | — | faultline-app | P0 | faultline-app | no |
| `integration_cached_boundary_observations_reused_no_reprobe` | Cached boundary observations are reused without re-probing | Mock ports + TrackingProbe | faultline-app | — | Cache reuse | — | app | — | — | faultline-app | P0 | faultline-app | no |
| `integration_full_localization_loop_with_mock_ports` | Full localization loop produces correct report with all fields | Mock ports + 10-commit seq | faultline-app | analysis.json | End-to-end correctness | — | app | — | analysis.json | faultline-app | P0 | faultline-app | no |
| `integration_flake_retries_attach_flake_signal` | Flake retries attach FlakeSignal to observations | Mock ports + retries=2 | faultline-app | — | Flake signal attachment | Req 3.1, 3.4, 3.5 | app | Req 3.1, 3.4, 3.5 | — | faultline-app | P0 | faultline-app | no |
| `integration_default_retries_no_flake_signal` | Default retries (0) produce no FlakeSignal | Mock ports + default policy | faultline-app | — | Default no signal | Req 3.6 | app | Req 3.6 | — | faultline-app | P1 | faultline-app | no |
| `integration_flaky_commit_majority_vote` | Flaky commit classified by majority vote | Mock ports + mixed results | faultline-app | — | Majority vote | — | app | — | — | faultline-app | P0 | faultline-app | no |
| `prop_probe_count_respects_max_probes` | Probe count never exceeds max_probes | `max_probes in 1..=10`, 20-commit seq | faultline-app | — | P3: Budget enforcement | — | app | — | — | faultline-app | P0 | faultline-app | yes |
| `prop_good_boundary_fail_yields_invalid_boundary` | Good boundary Fail always yields InvalidBoundary | `n in 3..=20` | faultline-app | — | P9: Boundary validation | Req 10.1–10.5 | app | Req 10.1–10.5 | — | faultline-app | P0 | faultline-app | yes |
| `prop_bad_boundary_pass_yields_invalid_boundary` | Bad boundary Pass always yields InvalidBoundary | `n in 3..=20` | faultline-app | — | P9: Boundary validation | Req 10.1–10.5 | app | Req 10.1–10.5 | — | faultline-app | P0 | faultline-app | yes |

---

## Entry Point Tier — CLI Tests

### faultline-cli

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `rejects_both_cmd_and_program` | Providing both --cmd and --program is rejected | CLI args | faultline-cli | — | Input validation | — | app | — | — | faultline-cli | P1 | faultline-cli | no |
| `rejects_neither_cmd_nor_program` | Providing neither --cmd nor --program is rejected | CLI args | faultline-cli | — | Input validation | — | app | — | — | faultline-cli | P1 | faultline-cli | no |
| `help_output_describes_all_flags` | --help output lists all expected flags | CLI --help | faultline-cli | — | Flag completeness | — | app | — | — | faultline-cli | P1 | faultline-cli | no |
| `help_output_describes_reproduce_subcommand` | --help output describes reproduce subcommand | CLI --help | faultline-cli | — | Subcommand completeness | Req 4.3 | app | Req 4.3 | — | faultline-cli | P1 | faultline-cli | no |
| `golden_cli_help` | Golden snapshot of CLI --help text | CLI --help | faultline-cli | help snapshot | Golden contract | Req 3.4 | app | Req 3.4 | — | faultline-cli | P0 | faultline-cli | yes |
| `exit_code_0_for_first_bad` | FirstBad outcome maps to exit code 0 | Hand-built outcome | faultline-cli | — | Exit code mapping | — | app | — | — | faultline-cli | P0 | faultline-cli | no |
| `exit_code_1_for_suspect_window` | SuspectWindow outcome maps to exit code 1 | Hand-built outcome | faultline-cli | — | Exit code mapping | — | app | — | — | faultline-cli | P0 | faultline-cli | no |
| `exit_code_3_for_inconclusive` | Inconclusive outcome maps to exit code 3 | Hand-built outcome | faultline-cli | — | Exit code mapping | — | app | — | — | faultline-cli | P0 | faultline-cli | no |
| `exit_code_2_for_execution_error` | ExecutionError maps to exit code 2 | OperatorCode | faultline-cli | — | Exit code mapping | — | app | — | — | faultline-cli | P0 | faultline-cli | no |
| `exit_code_4_for_invalid_input` | InvalidInput maps to exit code 4 | OperatorCode | faultline-cli | — | Exit code mapping | — | app | — | — | faultline-cli | P0 | faultline-cli | no |
| `all_exit_codes_are_distinct` | All exit codes are unique | All OperatorCodes | faultline-cli | — | Uniqueness | — | app | — | — | faultline-cli | P0 | faultline-cli | no |
| `rejects_resume_and_force` | --resume and --force are mutually exclusive | CLI args | faultline-cli | — | Mutual exclusion | — | app | — | — | faultline-cli | P1 | faultline-cli | no |
| `rejects_resume_and_fresh` | --resume and --fresh are mutually exclusive | CLI args | faultline-cli | — | Mutual exclusion | — | app | — | — | faultline-cli | P1 | faultline-cli | no |
| `rejects_force_and_fresh` | --force and --fresh are mutually exclusive | CLI args | faultline-cli | — | Mutual exclusion | — | app | — | — | faultline-cli | P1 | faultline-cli | no |
| `accepts_single_run_modes` | Single run mode flags accepted | CLI args | faultline-cli | — | Happy path | — | app | — | — | faultline-cli | P2 | faultline-cli | no |
| `accepts_valid_env_vars` | Valid --env KEY=VALUE accepted | CLI args | faultline-cli | — | Input validation | — | app | — | — | faultline-cli | P2 | faultline-cli | no |
| `accepts_env_var_with_equals_in_value` | --env FOO=bar=baz accepted | CLI args | faultline-cli | — | Input validation | — | app | — | — | faultline-cli | P2 | faultline-cli | no |
| `rejects_env_var_missing_equals` | --env without = is rejected | CLI args | faultline-cli | — | Input validation | — | app | — | — | faultline-cli | P1 | faultline-cli | no |
| `accepts_empty_env_list` | Empty --env list accepted | CLI args | faultline-cli | — | Edge case | — | app | — | — | faultline-cli | P2 | faultline-cli | no |
| `accepts_valid_shell_kinds` | Valid --shell values accepted | CLI args | faultline-cli | — | Input validation | — | app | — | — | faultline-cli | P2 | faultline-cli | no |
| `accepts_no_shell` | No --shell flag accepted | CLI args | faultline-cli | — | Default behavior | — | app | — | — | faultline-cli | P2 | faultline-cli | no |
| `rejects_unknown_shell` | Unknown --shell value rejected | CLI args | faultline-cli | — | Input validation | — | app | — | — | faultline-cli | P1 | faultline-cli | no |
| `prop_operator_code_exit_code_mapping` | Exit code mapping is consistent for all outcomes | `arb_localization_outcome()` | faultline-cli | — | P19: Exit code mapping | — | app | — | — | faultline-cli | P0 | faultline-cli | yes |
| `prop_cli_help_flag_completeness` | CLI --help lists all expected flags | Random seed | faultline-cli | — | P20: Flag completeness | — | app | — | — | faultline-cli | P0 | faultline-cli | yes |
| `smoke_cli_produces_artifacts` | End-to-end CLI run produces analysis.json and index.html | GitRepoBuilder + real CLI binary | faultline-cli | analysis.json, index.html | Smoke test | — | integration | — | analysis.json, index.html | faultline-cli | P0 | faultline-cli | yes |

---

## Integration Tier — BDD Scenarios

### faultline-render (BDD)

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `scenario_report_generation_end_to_end` | End-to-end report generation produces JSON + HTML + Markdown artifacts | Builder + TempDir | faultline-render | analysis.json, index.html, dossier.md | Artifact existence and content | Req 8.1 | integration | Req 8.1 | analysis.json, index.html, dossier.md | faultline-render | P0 | faultline-render | no |
| `scenario_resume_rerender_consistency` | Re-rendering from cached report produces consistent output | Builder + serialize/deserialize | faultline-render | analysis.json, index.html | Re-render consistency | Req 8.1 | integration | Req 8.1 | analysis.json, index.html | faultline-render | P0 | faultline-render | no |

### faultline-sarif (BDD)

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `scenario_export_surfaces_sarif_and_junit_consistency` | SARIF + JUnit from same report contain consistent suspect surface data | Builder with suspect surface | faultline-sarif, faultline-junit | SARIF JSON, JUnit XML | Cross-export consistency | Req 8.1 | integration | Req 8.1 | SARIF JSON, JUnit XML | faultline-sarif, faultline-junit | P0 | faultline-sarif | no |

---

## Tooling Tier — Xtask Tests

### xtask

| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs | Tier | Req IDs | Contract | Mutation Surface | Crit | Owner | Review |
|----------|---------|-------------------|----------|----------|-----------|------|------|---------|----------|------------------|------|-------|--------|
| `xtask_help_lists_all_subcommands` | Xtask --help lists all expected subcommands | CLI --help | xtask | — | Subcommand completeness | Req 5.2, 5.5, 10.1 | app | Req 5.2, 5.5, 10.1 | — | xtask | P1 | xtask | no |
| `scaffold_help_lists_all_kinds` | Scaffold --help lists all scaffold kinds | CLI --help | xtask | — | Subcommand completeness | — | app | — | — | xtask | P2 | xtask | no |
| `valid_crate_names` | Valid crate names accepted by scaffold | Fixed names | xtask | — | Input validation | — | app | — | — | xtask | P2 | xtask | no |
| `invalid_crate_names` | Invalid crate names rejected by scaffold | Fixed names | xtask | — | Input validation | — | app | — | — | xtask | P1 | xtask | no |
| `valid_sections` | Valid doc sections accepted | Fixed sections | xtask | — | Input validation | — | app | — | — | xtask | P2 | xtask | no |
| `invalid_sections` | Invalid doc sections rejected | Fixed sections | xtask | — | Input validation | — | app | — | — | xtask | P1 | xtask | no |
| `empty_strings_rejected` | Empty strings rejected by validators | Empty string | xtask | — | Input validation | — | app | — | — | xtask | P1 | xtask | no |
| `slugify_works` | Slugify produces correct kebab-case | Fixed strings | xtask | — | String transformation | — | app | — | — | xtask | P2 | xtask | no |
| `extract_test_names_finds_standard_tests` | Test name extraction finds #[test] functions | Sample source | xtask | — | Extraction correctness | — | app | — | — | xtask | P1 | xtask | no |
| `extract_test_names_finds_proptest_fns` | Test name extraction finds proptest functions | Sample source | xtask | — | Extraction correctness | — | app | — | — | xtask | P1 | xtask | no |
| `extract_index_entries_parses_table` | Scenario index table parsing works | Sample Markdown table | xtask | — | Parse correctness | — | app | — | — | xtask | P1 | xtask | no |
| `check_consistency_reports_symmetric_difference` | Consistency check reports missing/extra entries | Mismatched sets | xtask | — | Diff correctness | — | app | — | — | xtask | P0 | xtask | no |
| `check_consistency_perfect_match_is_ok` | Consistency check passes for matching sets | Identical sets | xtask | — | Happy path | — | app | — | — | xtask | P1 | xtask | no |
| `pattern_entry_structural_completeness` | Pattern catalog entries have all required fields | Pattern catalog file | xtask | — | P37: Structural completeness | — | app | — | — | xtask | P1 | xtask | no |
| `scenario_entry_structural_completeness` | Scenario index entries have all required fields | Scenario index file | xtask | — | P38: Structural completeness | — | app | — | — | xtask | P1 | xtask | no |
| `scenario_schema_drift_detected_on_modified_report` | Schema drift detected when report type changes | Modified schema | xtask | analysis-report.schema.json | P45: Schema drift detection | Req 8.3 | app | Req 8.3 | analysis-report.schema.json | xtask | P0 | xtask | no |
| `scenario_schema_no_drift_when_matching` | No schema drift when schema matches types | Correct schema | xtask | analysis-report.schema.json | Schema consistency | Req 8.3 | app | Req 8.3 | analysis-report.schema.json | xtask | P1 | xtask | no |
| `scenario_schema_drift_field_removal_detected` | Schema drift detected on field removal | Schema with removed field | xtask | analysis-report.schema.json | Schema drift detection | Req 8.3 | app | Req 8.3 | analysis-report.schema.json | xtask | P0 | xtask | no |
| `tool_detection_error_messages_known_tools` | Tool detection error messages are correct for known tools | Known tool names | xtask | — | P44: Error messages | Req 5.7 | app | Req 5.7 | — | xtask | P1 | xtask | no |
| `ci_failure_messages_identify_broken_contract` | CI failure messages identify the broken contract | Contract names | xtask | — | P46: CI messages | Req 8.7 | app | Req 8.7 | — | xtask | P0 | xtask | no |
| `prop_symmetric_difference_is_exact` | Symmetric difference reports exact missing/extra entries | `arb_test_name_set()` × 2 | xtask | — | P39: Atlas consistency | Req 2.5, 8.4 | app | Req 2.5, 8.4 | — | xtask | P0 | xtask | yes |
| `prop_identical_sets_yield_ok` | Identical test sets yield Ok | `arb_test_name_set()` | xtask | — | P39: Atlas consistency | Req 2.5, 8.4 | app | Req 2.5, 8.4 | — | xtask | P0 | xtask | yes |
| `prop_extract_and_check_round_trip` | Extract + check round-trips correctly | Random test names | xtask | — | P39: Atlas consistency | Req 2.5, 8.4 | app | Req 2.5, 8.4 | — | xtask | P0 | xtask | yes |
| `prop_scaffold_crate_generation` | Scaffold crate generates valid structure | `arb_crate_suffix()` | xtask | — | P47: Scaffold generation | Req 10.2 | app | Req 10.2 | — | xtask | P0 | xtask | yes |
| `prop_scaffold_adr_sequential_numbering` | Scaffold ADR uses sequential numbering | `existing_count in 0..20` | xtask | — | P48: ADR numbering | Req 10.3 | app | Req 10.3 | — | xtask | P0 | xtask | yes |
| `prop_scaffold_scenario_creates_stub_and_index` | Scaffold scenario creates test stub and index entry | Random names | xtask | — | P49: Scaffold file generation | Req 10.4, 10.5 | app | Req 10.4, 10.5 | — | xtask | P0 | xtask | yes |
| `prop_scaffold_doc_creates_file_and_summary_entry` | Scaffold doc creates file and SUMMARY entry | Random sections | xtask | — | P49: Scaffold file generation | Req 10.4, 10.5 | app | Req 10.4, 10.5 | — | xtask | P0 | xtask | yes |
| `prop_scaffold_rejects_invalid_crate_names` | Scaffold rejects invalid crate names | Invalid name patterns | xtask | — | P50: Input validation | Req 10.6 | app | Req 10.6 | — | xtask | P0 | xtask | yes |
| `prop_scaffold_rejects_empty_adr_titles` | Scaffold rejects empty ADR titles | Empty/whitespace titles | xtask | — | P50: Input validation | Req 10.6 | app | Req 10.6 | — | xtask | P0 | xtask | yes |
| `prop_scaffold_rejects_empty_scenario_names` | Scaffold rejects empty scenario names | Empty/whitespace names | xtask | — | P50: Input validation | Req 10.6 | app | Req 10.6 | — | xtask | P0 | xtask | yes |
| `prop_scaffold_rejects_invalid_doc_sections` | Scaffold rejects invalid doc sections | Invalid section names | xtask | — | P50: Input validation | Req 10.6 | app | Req 10.6 | — | xtask | P0 | xtask | yes |
| `prop_schema_drift_detection` | Schema drift detection works for any schema change | Generated schemas | xtask | analysis-report.schema.json | P45: Schema drift | Req 8.3 | app | Req 8.3 | analysis-report.schema.json | xtask | P0 | xtask | yes |
| `split_fragment_works` | Fragment splitting handles path#anchor, path-only, and anchor-only | Fixed strings | xtask | — | String parsing | — | app | — | — | xtask | P2 | xtask | no |
| `extract_inline_links` | Inline Markdown links extracted correctly | Fixed Markdown line | xtask | — | Link extraction | — | app | — | — | xtask | P2 | xtask | no |
| `extract_reference_links` | Reference-style Markdown links extracted correctly | Fixed Markdown line | xtask | — | Link extraction | — | app | — | — | xtask | P2 | xtask | no |
| `extract_skips_external` | External URLs identified and skipped | Fixed Markdown line | xtask | — | Link filtering | — | app | — | — | xtask | P2 | xtask | no |
| `check_file_finds_broken_links` | Broken local links detected in Markdown file | TempDir + Markdown file | xtask | — | Link checking | Req 10.2 | app | Req 10.2 | — | xtask | P1 | xtask | no |
| `check_file_skips_external_and_anchors` | External URLs and anchor links skipped during checking | TempDir + Markdown file | xtask | — | Link filtering | Req 10.2 | app | Req 10.2 | — | xtask | P2 | xtask | no |
| `check_links_on_clean_dir` | Clean directory passes link check | TempDir + valid links | xtask | — | Happy path | Req 10.2 | app | Req 10.2 | — | xtask | P1 | xtask | no |
| `check_links_reports_broken` | Broken links reported as error | TempDir + broken link | xtask | — | Error reporting | Req 10.2 | app | Req 10.2 | — | xtask | P1 | xtask | no |
| `collect_markdown_files_finds_root_and_docs` | Markdown file collection finds root and docs/ files | TempDir + nested Markdown | xtask | — | File discovery | Req 10.2 | app | Req 10.2 | — | xtask | P1 | xtask | no |
| `fixture_repo_has_three_commits` | Smoke test fixture repo has expected commit count | GitRepoBuilder | xtask | — | Fixture correctness | Req 10.1 | app | Req 10.1 | — | xtask | P1 | xtask | no |
| `extract_test_names` | Test name extraction function works correctly | Sample source | xtask | — | Extraction correctness | — | app | — | — | xtask | P2 | xtask | no |
| `my_test_one` | Test data: standard #[test] function detected by scanner | Test data in scenarios.rs | xtask | — | Scanner test data | — | app | — | — | xtask | P2 | xtask | no |
| `my_test_two` | Test data: second #[test] function detected by scanner | Test data in scenarios.rs | xtask | — | Scanner test data | — | app | — | — | xtask | P2 | xtask | no |
| `name` | Test data: proptest function name detected by scanner | Test data in scenarios.rs | xtask | — | Scanner test data | — | app | — | — | xtask | P2 | xtask | no |
| `prop_something` | Test data: proptest function detected by scanner | Test data in scenarios.rs | xtask | — | Scanner test data | — | app | — | — | xtask | P2 | xtask | no |
| `schema_drift_error_message_format` | Schema drift error message has correct format | Modified schema | xtask | — | Error message format | Req 8.3 | app | Req 8.3 | — | xtask | P1 | xtask | no |
| `tool_detection_error_message_format` | Tool detection error message format is correct | Random tool names | xtask | — | P44: Error message format | Req 5.7 | app | Req 5.7 | — | xtask | P0 | xtask | yes |
| `ci_contract_broken_message_contains_contract_name` | CI contract broken message contains contract name | Random contract names | xtask | — | P46: CI message format | Req 8.7 | app | Req 8.7 | — | xtask | P0 | xtask | yes |
| `ci_golden_failure_message_contains_artifact_and_docs` | CI golden failure message contains artifact and docs refs | Random artifacts | xtask | — | P46: CI message format | Req 8.7 | app | Req 8.7 | — | xtask | P0 | xtask | yes |
| `ci_missing_scenario_message_contains_files_and_docs` | CI missing scenario message contains file refs and docs | Random file names | xtask | — | P46: CI message format | Req 8.7 | app | Req 8.7 | — | xtask | P0 | xtask | yes |
