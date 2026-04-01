# Playbook: Deciding Test Technique

This playbook helps you choose the right testing technique for a given change. faultline uses five complementary techniques; each has a sweet spot.

## Decision Tree

```text
Is the behavior a universal invariant over all valid inputs?
  YES → Property test
  NO  ↓

Is the behavior a specific artifact format or CLI output?
  YES → Golden/snapshot test
  NO  ↓

Does the code parse untrusted or arbitrary input?
  YES → Fuzz test (+ unit tests for known edge cases)
  NO  ↓

Is the behavior a concrete example, edge case, or integration flow?
  YES → Unit test or BDD scenario
  NO  ↓

Is the code in a mutation-targeted crate?
  YES → Ensure existing tests catch mutants (run cargo xtask mutants --crate <name>)
  NO  → Unit test covering the core behavior
```

## Technique Reference

### Property Tests (`proptest`)

**Use when:** You can state a universal rule that holds for *all* valid inputs.

**Sweet spot:** Pure domain logic in `faultline-types`, `faultline-localization`, `faultline-surface`, `faultline-codes`.

**Examples from faultline:**
- `prop_binary_narrowing_selects_valid_midpoint` — for any revision sequence, the midpoint is always within bounds
- `prop_suspect_surface_sorted_deterministic` (P43) — for any set of path changes, ranking is sorted and identical across calls
- `prop_compare_runs_is_total` (P53) — for any two reports, `compare_runs` never panics

**Configuration:** Minimum 100 cases. Use `ProptestConfig { cases: 100, .. ProptestConfig::default() }`.

**When NOT to use:** When the behavior depends on external state (filesystem, Git repo, network) or when the invariant is too weak to be meaningful.

### Unit Tests

**Use when:** You need to verify a specific example, edge case, error condition, or integration flow with known inputs and expected outputs.

**Sweet spot:** Error handling paths, boundary values, specific input/output pairs, adapter logic with mocked or in-memory dependencies.

**Examples from faultline:**
- `test_empty_input_returns_empty_surface` — edge case for empty path list
- `test_shell_script_escaping` — specific escaping behavior for `ReproductionCapsule`
- `test_codeowners_malformed_file` — error handling for bad CODEOWNERS content

**When NOT to use:** When you find yourself writing many similar tests that differ only in input values — that's a sign you want a property test instead.

### Golden/Snapshot Tests (`insta`)

**Use when:** The output is a structured artifact (JSON, HTML, CLI help text) and you want to detect any change to its format or content.

**Sweet spot:** `faultline-render` (JSON + HTML artifacts), `faultline-cli` (`--help` text), `faultline-sarif` and `faultline-junit` (export format).

**Examples from faultline:**
- `golden_analysis_json` — snapshots the canonical JSON report
- `golden_index_html` — snapshots the HTML report
- `golden_cli_help` — snapshots CLI `--help` output

**Workflow:** When a golden test fails, run `cargo insta review` to inspect and accept or reject the change.

**When NOT to use:** For internal data structures or intermediate values that aren't part of a public contract.

### Fuzz Tests (`cargo-fuzz`)

**Use when:** The code parses, deserializes, or processes untrusted or arbitrary input and must not panic.

**Sweet spot:** Deserialization paths (`faultline-store` JSON loading), Git output parsing (`faultline-git`), HTML escaping (`faultline-render`), CLI argument parsing (`faultline-cli`).

**Examples from faultline:**
- `fuzz_git_diff_parse` — arbitrary byte strings as `git diff --name-status` output
- `fuzz_store_json` — arbitrary byte strings as `observations.json`
- `fuzz_html_escape` — adversarial strings through the HTML escaper
- `fuzz_sarif_export` / `fuzz_junit_export` — arbitrary report JSON through export adapters

**Run with:** `cargo xtask fuzz --duration 60`

**When NOT to use:** For pure domain logic that only receives well-typed Rust values (use property tests instead).

### Mutation Tests (`cargo-mutants`)

**Use when:** You want to verify that the existing test suite actually catches meaningful code changes in a crate.

**Sweet spot:** Domain crates (`faultline-localization`, `faultline-surface`) and adapter crates with complex logic.

**Run with:**
```bash
cargo xtask mutants                              # all configured crates
cargo xtask mutants --crate faultline-localization  # single crate
```

**When NOT to use:** As a primary testing technique — mutation testing validates your other tests, it doesn't replace them.

## Combining Techniques

Most changes need more than one technique. A typical domain change:

1. **Property test** for the universal invariant
2. **Unit test** for specific edge cases the property doesn't cover well
3. **Golden test update** if the change affects artifact output
4. **Mutation test run** to verify coverage

A typical adapter change:

1. **Unit test** for the happy path and error cases
2. **Fuzz test** if the adapter parses external input
3. **Golden test update** if the adapter produces a public artifact
4. **Property test** if there's a meaningful invariant over the adapter's output

## Cross-References

| Document | Purpose |
|----------|---------|
| [TESTING.md](../../TESTING.md) | Full verification guide with how-to sections |
| [Verification Matrix](../verification-matrix.md) | Per-crate technique assignments |
| [Pattern Catalog — Proof-Carrying Change](../patterns/catalog.md) | Every domain change needs a test |
| [Scenario Atlas](../scenarios/scenario_index.md) | All tests indexed by behavior |
