# Worked Example: Handling a Flaky Test with Flake-Aware Probing

This walkthrough demonstrates how to use faultline's flake-aware probing to get reliable results when your predicate is flaky — that is, it sometimes passes and sometimes fails on the same commit.

## Scenario

You're bisecting a test failure, but the failing test is known to be intermittent. A standard faultline run produces a `SuspectWindow` with low confidence because the predicate gives inconsistent results at boundary commits.

## Step 1: Run faultline without flake detection (the problem)

```bash
faultline-cli \
  --repo . \
  --good v1.0 \
  --bad main \
  --cmd "cargo test --lib test_network_timeout"
```

The report shows:

```
outcome  SuspectWindow  confidence=30(low)
reasons  NonMonotonicEvidence
```

The low confidence and non-monotonic evidence suggest the predicate is flaky — a commit that should pass is occasionally failing, or vice versa.

## Step 2: Enable flake-aware probing

Re-run with `--retries` and `--stability-threshold`:

```bash
faultline-cli \
  --repo . \
  --good v1.0 \
  --bad main \
  --cmd "cargo test --lib test_network_timeout" \
  --retries 3 \
  --stability-threshold 0.75
```

This tells faultline:
- **`--retries 3`**: Probe each commit up to 4 times total (1 initial + 3 retries).
- **`--stability-threshold 0.75`**: A commit is considered stable if at least 75% of its probe results agree.

## Step 3: Understand the FlakeSignal

With retries enabled, each observation in the report now includes a `FlakeSignal`:

```json
{
  "commit": "abc1234",
  "class": "Fail",
  "flake_signal": {
    "total_runs": 4,
    "pass_count": 1,
    "fail_count": 3,
    "skip_count": 0,
    "indeterminate_count": 0,
    "is_stable": true
  }
}
```

In this example, the commit was probed 4 times: 3 failures and 1 pass. Since 75% (3/4) of results agree on `Fail`, and the stability threshold is 0.75, the signal is classified as **stable**. The majority-vote result (`Fail`) is used for localization.

## Step 4: Read the improved outcome

The flake-aware run produces a cleaner result:

```
outcome  FirstBad  last_good=def5678 first_bad=abc1234 confidence=90(high)
```

By retrying and using majority vote, faultline filtered out the noise from the flaky predicate and pinpointed the regression commit with high confidence.

## Step 5: Check for unstable observations

If some commits remain unstable even after retries, the report flags them. Look for observations where `flake_signal.is_stable == false`:

```json
{
  "commit": "xyz9999",
  "class": "Fail",
  "flake_signal": {
    "total_runs": 4,
    "pass_count": 2,
    "fail_count": 2,
    "skip_count": 0,
    "indeterminate_count": 0,
    "is_stable": false
  }
}
```

Here, results are split 50/50 — below the 0.75 threshold. This observation degrades the overall confidence score. The report will note this in the ambiguity reasons.

## Step 6: Tune the threshold

The right threshold depends on your predicate's flakiness:

| Threshold | Meaning | Use when |
|-----------|---------|----------|
| `1.0` (default) | All retries must agree | Predicate is reliable |
| `0.75` | 75% agreement required | Occasional flakes |
| `0.5` | Simple majority wins | Highly flaky predicate |

Lower thresholds tolerate more noise but may misclassify borderline commits. Start with `0.75` and adjust based on results.

## Step 7: Reproduce a specific probe

If you want to manually verify a flaky commit, use the reproduction capsule:

```bash
faultline reproduce --run-dir regression-report --commit abc1234 --shell
```

This emits a shell script that checks out the exact commit, sets the environment, and runs the predicate — so you can reproduce the probe conditions exactly.

## Key takeaways

- Default behavior (retries=0) probes each commit once. Enable `--retries` when you suspect flakiness.
- The `FlakeSignal` records exactly how many times each result was observed, giving you full transparency.
- Unstable observations (below the stability threshold) degrade confidence — faultline never hides uncertainty.
- The stability threshold is a dial: higher values demand more consistency, lower values tolerate more noise.
- Combine flake-aware probing with reproduction capsules to manually verify suspicious commits.
