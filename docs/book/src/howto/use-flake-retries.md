# How to Use --retries and --stability-threshold

This guide explains how to configure faultline's flake-aware probing when your predicate produces inconsistent results.

## When to use flake retries

Enable retries when:
- Your predicate is known to be flaky (intermittent pass/fail on the same code)
- A previous run produced `SuspectWindow` with `NonMonotonicEvidence`
- You see low confidence scores that don't match your expectations

## Step 1: Add --retries

The `--retries` flag sets how many additional probe attempts faultline makes per commit (on top of the initial probe):

```bash
faultline-cli \
  --repo . --good v1.0 --bad main \
  --cmd "cargo test" \
  --retries 3
```

This probes each commit up to 4 times (1 initial + 3 retries). The final classification uses majority vote across all attempts.

## Step 2: Set --stability-threshold

The `--stability-threshold` flag (0.0–1.0) controls how much agreement is required to consider a result stable:

```bash
faultline-cli \
  --repo . --good v1.0 --bad main \
  --cmd "cargo test" \
  --retries 3 \
  --stability-threshold 0.75
```

A commit is classified as stable if the proportion of the most-frequent result meets or exceeds the threshold. With threshold 0.75 and 4 runs, at least 3 must agree.

## Step 3: Interpret FlakeSignal in the report

Each observation in the report includes a `flake_signal` when retries are enabled:

```json
{
  "total_runs": 4,
  "pass_count": 3,
  "fail_count": 1,
  "skip_count": 0,
  "indeterminate_count": 0,
  "is_stable": true
}
```

- `is_stable: true` — the result met the threshold; the majority-vote class is used
- `is_stable: false` — the result did not meet the threshold; confidence is degraded

## Choosing a threshold

| Threshold | Behavior | Best for |
|-----------|----------|----------|
| `1.0` (default) | All runs must agree | Reliable predicates |
| `0.75` | 75% agreement | Occasional flakes |
| `0.5` | Simple majority | Highly flaky predicates |

## Notes

- With `--retries 0` (the default), no `FlakeSignal` is attached to observations and no retry logic runs.
- Higher retry counts increase accuracy but also increase total probe time proportionally.
- Unstable observations always degrade the overall confidence score — faultline never hides uncertainty.
