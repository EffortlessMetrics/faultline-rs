# How Flake-Aware Probing Works

This page explains faultline's flake detection mechanism — how it handles unreliable predicates through retries, stability classification, and confidence degradation.

## The problem

In an ideal world, a predicate (test or build command) produces the same result every time it runs on the same code. In practice, predicates can be flaky: a test might pass 80% of the time and fail 20% due to timing issues, network dependencies, or resource contention.

Without flake detection, a single unlucky probe can misclassify a commit. A good commit that happens to fail looks like a regression. A bad commit that happens to pass looks clean. This produces non-monotonic evidence (a fail appearing before a pass in sequence order), which forces faultline to report a `SuspectWindow` with low confidence instead of pinpointing the exact regression.

## The solution: retry and vote

Flake-aware probing addresses this by probing each commit multiple times and using majority vote to determine the classification.

### FlakePolicy

The operator controls flake detection through two parameters:

- **`retries`** (default: 0) — How many additional probe attempts to make per commit. With `retries = 3`, each commit is probed up to 4 times.
- **`stability_threshold`** (default: 1.0) — The minimum proportion of the most-frequent result required to classify the commit as stable.

When `retries = 0` (the default), no retry logic runs and no `FlakeSignal` is attached to observations. This preserves backward compatibility and avoids unnecessary overhead for reliable predicates.

### Retry loop

When retries are enabled, the app orchestrator runs this loop for each commit:

1. Probe the commit (initial attempt)
2. If `retries > 0`, probe again up to `retries` additional times
3. Collect all results into a set of `ObservationClass` values
4. Compute the `FlakeSignal` from the result set

### FlakeSignal computation

The `FlakeSignal` summarizes the retry results:

```
FlakeSignal {
    total_runs: 4,      // 1 initial + 3 retries
    pass_count: 3,       // how many times the predicate passed
    fail_count: 1,       // how many times it failed
    skip_count: 0,       // how many times it returned exit 125
    indeterminate_count: 0,  // how many times it timed out
    is_stable: true,     // whether the result meets the threshold
}
```

The `is_stable` flag is computed as:

```
is_stable = (max(pass_count, fail_count, skip_count, indeterminate_count) / total_runs)
            >= stability_threshold
```

The counts always sum to `total_runs`.

### Majority vote classification

The final `ObservationClass` for the commit is the most-frequent result. In the example above, `Pass` (3 out of 4) wins. This filtered result is what the localization engine uses for binary narrowing.

## Confidence degradation

Unstable observations (where `is_stable == false`) degrade the overall confidence score. This is a deliberate design choice: faultline never hides uncertainty.

If a session reaches a `FirstBad` or `SuspectWindow` outcome but one or more observations have `is_stable == false`, the confidence score is strictly lower than it would be if all observations were stable. This signals to the operator that some probe results were unreliable and the conclusion should be treated with appropriate caution.

## Interaction with the localization engine

The localization engine (`LocalizationSession`) receives observations with their `FlakeSignal` attached. It uses the majority-vote classification for binary narrowing but records the full signal for reporting. The engine's confidence scoring accounts for instability:

- All observations stable → normal confidence rules apply
- Any observation unstable → confidence is reduced

This means flake-aware probing improves accuracy (by filtering noise through retries) while maintaining honesty (by degrading confidence when results are ambiguous).

## When to use flake detection

| Situation | Recommendation |
|-----------|---------------|
| Reliable predicate, consistent results | Leave defaults (`--retries 0`) |
| Occasional flakes, mostly consistent | `--retries 2 --stability-threshold 0.75` |
| Highly flaky predicate | `--retries 4 --stability-threshold 0.5` |
| Previous run showed NonMonotonicEvidence | Enable retries and re-run |

Higher retry counts increase accuracy but also increase total execution time proportionally. Choose the minimum retries needed to get stable results.
