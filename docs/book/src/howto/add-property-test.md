# Adding a New Property Test

This guide walks through creating a property-based test using `proptest`, wiring it to a requirement, and updating the Scenario Atlas.

## Step 1: Choose the right crate

Property tests belong in the crate that owns the logic under test. Domain crates are the primary targets:

| Crate | What to test |
|-------|-------------|
| `faultline-localization` | Narrowing logic, outcome classification, boundary detection |
| `faultline-surface` | Path bucketing, change classification |
| `faultline-types` | Serialization round-trips, value object invariants |
| `faultline-codes` | Enum parsing, display formatting |

Adapter crates (`faultline-render`, `faultline-sarif`, `faultline-junit`) also use property tests for structural validity of their output formats.

## Step 2: Add proptest dependency

If the crate doesn't already have `proptest` in its dev-dependencies, add it:

```toml
# crates/faultline-<name>/Cargo.toml
[dev-dependencies]
proptest = "1.5"
```

## Step 3: Write the property test

Create or extend the `#[cfg(test)]` module in the crate's `src/lib.rs` (or a dedicated test file).

A property test has three parts:

1. **Generators** — produce arbitrary valid inputs
2. **Action** — run the code under test
3. **Assertions** — verify the property holds for all generated inputs

Example: testing that serialization round-trips preserve data:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        // Feature: <feature-name>, Property N: <property-name>
        // **Validates: Requirements X.Y**
        #[test]
        fn prop_round_trip_serialization(value in arb_my_type()) {
            let json = serde_json::to_string(&value).unwrap();
            let deserialized: MyType = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(value, deserialized);
        }
    }
}
```

Key conventions:

- Use `ProptestConfig { cases: 100, .. }` as the minimum iteration count
- Prefix test function names with `prop_`
- Add the `// **Validates: Requirements X.Y**` comment linking to the requirement
- Add the `// Feature: <name>, Property N: <name>` comment for traceability

## Step 4: Write smart generators

Good generators constrain inputs to the valid domain rather than generating arbitrary data and filtering. Use `prop_oneof!` for enum variants and `prop::collection::vec` for bounded collections.

```rust
fn arb_confidence() -> impl Strategy<Value = Confidence> {
    prop_oneof![
        Just(Confidence::high()),
        Just(Confidence::medium()),
        Just(Confidence::low()),
    ]
}

fn arb_commit_id() -> impl Strategy<Value = CommitId> {
    "[a-f0-9]{8,40}".prop_map(CommitId)
}
```

If the crate has shared generators, place them in `faultline-fixtures/src/arb.rs`.

## Step 5: Run the test

```bash
cargo test -p faultline-<crate> -- prop_
```

If the test fails, proptest will print a minimal counterexample. Investigate whether:

1. The test logic is wrong — fix the test
2. The code has a bug — fix the code
3. The specification is unclear — ask a maintainer before changing acceptance criteria

Do not suppress or weaken a failing property test without maintainer approval (see the Human Review Gate pattern in the [Pattern Catalog](../../patterns/catalog.md)).

## Step 6: Update the Scenario Atlas

Every test must have a corresponding entry in `docs/scenarios/scenario_index.md`. Add a row to the appropriate crate tier table:

```markdown
| prop_my_new_property | Brief problem description | `arb_my_type()` | faultline-<crate> | — | Property holds for all valid inputs | Req X.Y |
```

The seven columns are:

1. **Scenario** — test function name
2. **Problem** — one-sentence description of what it verifies
3. **Fixture/Generator** — the proptest strategy or fixture builder used
4. **Crate(s)** — which crate(s) the test exercises
5. **Artifact** — output artifact validated (or `—` for pure logic tests)
6. **Invariant** — the property or invariant asserted
7. **Refs** — related requirements, ADRs, or doc references

## Step 7: Verify CI passes

Run the fast CI tier to confirm everything compiles and passes:

```bash
cargo xtask ci-fast
```

Or using the just alias:

```bash
just ci
```

CI will also verify that your new test has a scenario index entry (the scenario atlas consistency check).
