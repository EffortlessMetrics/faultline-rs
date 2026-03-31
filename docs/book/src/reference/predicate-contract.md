# Predicate Contract

The predicate is the command that faultline runs at each candidate revision to determine whether the regression is present. It must follow a strict exit code contract.

For the full contract, see [`docs/predicate-contract.md`](../../../../docs/predicate-contract.md) in the repository root.

## Exit classification

| Exit code | Classification | Meaning |
|-----------|---------------|---------|
| `0` | Pass | The predicate succeeded — this revision is good |
| `125` | Skip | The revision is untestable (e.g., won't compile, missing dependency) |
| Any other non-zero | Fail | The predicate failed — this revision exhibits the regression |
| Timeout | Indeterminate | The predicate did not complete within `--timeout-seconds` |

## Required behavior

The predicate must express exactly one of the four classifications above. Faultline determines the classification from the process exit code and timeout status.

## Guidance for writing predicates

Best results come from predicates that are:

- **Deterministic** — same revision always produces the same result
- **Local** — no dependency on external mutable services (databases, APIs)
- **Isolated** — no side effects that persist between runs
- **Narrow** — test only the specific behavior under investigation, not the entire test suite

When the predicate is noisy (non-deterministic), faultline will reduce confidence and may return a `SuspectWindow` instead of an exact `FirstBad` commit.

## Examples

A simple test predicate:

```bash
faultline-cli --good v1.0 --bad main --cmd "cargo test --lib my_module"
```

A build predicate:

```bash
faultline-cli --good v1.0 --bad main --cmd "cargo build" --kind build
```

A predicate that skips revisions missing a file:

```bash
#!/bin/bash
# test-with-skip.sh
if [ ! -f src/feature.rs ]; then
    exit 125  # skip — file doesn't exist yet
fi
cargo test --lib feature_tests
```

```bash
faultline-cli --good v1.0 --bad main --program ./test-with-skip.sh
```
