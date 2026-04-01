# Playbook: Reviewing Failing Property Tests

This playbook walks you through investigating and resolving a failing property test in the faultline workspace.

## When to Use

A property test has failed in CI or locally, producing a counterexample. The Human Review Gate pattern (see [Pattern Catalog](../patterns/catalog.md)) requires human triage — automated suppression is never acceptable.

## Step 1: Read the Counterexample

Property tests (via `proptest`) print the minimal shrunk counterexample that triggers the failure. Look for output like:

```text
proptest: Minimal failing input:
  input = SomeStruct { field: 42, ... }
```

Copy the counterexample. It tells you *what* input broke the invariant.

## Step 2: Reproduce Locally

Run the failing test in isolation:

```bash
cargo test -p <crate> <test_name> -- --nocapture
```

If the test uses randomness and you need the exact seed, check for a `proptest-regressions/` file in the crate directory — `proptest` persists failing seeds there automatically.

## Step 3: Determine Root Cause

There are three possible outcomes:

### A. Bug in the Code

The property is correct, but the implementation violates it. This is the most common case.

**Action:** Fix the implementation. Do not weaken the property. Run the full test suite to confirm the fix doesn't break other invariants:

```bash
cargo test -p <crate>
cargo xtask ci-fast
```

### B. Bug in the Test

The property itself is wrong — it asserts something that was never intended or misinterprets the specification.

**Action:** Fix the test with a clear commit message explaining *why* the property was wrong. Include the counterexample in the commit message for context. Get a second reviewer to confirm the property change is justified.

### C. Specification Gap

The property is technically correct per the acceptance criteria, but the acceptance criteria are incomplete or ambiguous. The counterexample reveals a case the spec didn't consider.

**Action:** Do not change the test or the code unilaterally. Open a discussion (issue or PR comment) describing:
- The counterexample
- What the current spec says
- What the counterexample implies the spec should say
- Your recommendation

Wait for maintainer input before proceeding.

## Step 4: Add a Regression Test

After fixing the code or the property, add the counterexample as a dedicated unit test to prevent recurrence:

```rust
#[test]
fn regression_issue_NNN_counterexample() {
    // Counterexample from prop_xxx failure: input = ...
    let input = /* exact counterexample values */;
    let result = function_under_test(input);
    assert!(/* the invariant holds */);
}
```

Add the regression test to the [Scenario Atlas](../scenarios/scenario_index.md).

## Step 5: Verify and Submit

1. Run `cargo test -p <crate>` to confirm the fix.
2. Run `cargo xtask ci-fast` for the full suite.
3. If golden snapshots changed, run `cargo insta review`.
4. Submit the PR with the fix, the regression test, and the scenario atlas entry.

## Rules

- Never delete or skip a failing property test.
- Never reduce iteration count below 100.
- Never weaken assertions to make a test pass without maintainer approval.
- Never mark property test failures as "expected."
- The `proptest-regressions/` files should be committed — they ensure the counterexample is re-tested on every run.

## Escalation

If you cannot determine the root cause after investigation, escalate per the [Maintainers guide](../../MAINTAINERS.md):
- Domain logic failures → Level 2 review (domain-familiar reviewer)
- Schema or API implications → Level 3 review (maintainer)
- Architectural implications → Level 4 review (two maintainers + ADR)

## Cross-References

| Document | Purpose |
|----------|---------|
| [Pattern Catalog — Human Review Gate](../patterns/catalog.md) | The pattern this playbook implements |
| [TESTING.md](../../TESTING.md) | How to add property tests, CI tiers |
| [MAINTAINERS.md](../../MAINTAINERS.md) | Escalation paths and review expectations |
| [Verification Matrix](../verification-matrix.md) | Which crates require property tests |
