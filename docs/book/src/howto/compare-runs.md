# How to Compare Successive Faultline Runs

This guide explains how to use the `diff-runs` subcommand to compare two faultline runs and answer "did we narrow the problem?"

## When to use diff-runs

Use run comparison when:
- You re-ran faultline after fixing a flaky test and want to see if the outcome improved
- You increased the probe budget and want to see if the window narrowed
- You want to track regression investigation progress across multiple runs

## Step 1: Run faultline twice

Run faultline with different parameters or at different times, saving each to a separate output directory:

```bash
# First run
faultline-cli --repo . --good v1.0 --bad main \
  --cmd "cargo test" --output-dir run-1

# Second run (e.g., with more probes)
faultline-cli --repo . --good v1.0 --bad main \
  --cmd "cargo test" --max-probes 40 --output-dir run-2
```

## Step 2: Compare the runs

```bash
faultline diff-runs --left run-1 --right run-2
```

This produces a human-readable summary showing what changed between the two runs.

## Step 3: Read the comparison output

The comparison reports:

| Field | Meaning |
|-------|---------|
| `outcome_changed` | Whether the outcome type changed (e.g., SuspectWindow → FirstBad) |
| `confidence_delta` | Change in confidence score (positive = improved) |
| `window_width_delta` | Change in suspect window width (negative = narrower, better) |
| `probes_reused` | How many probe results were identical across both runs |
| `suspect_paths_added` | New paths in the suspect surface |
| `suspect_paths_removed` | Paths no longer in the suspect surface |

## Step 4: Get JSON output

For machine consumption, add `--json`:

```bash
faultline diff-runs --left run-1 --right run-2 --json
```

This outputs the full `RunComparison` struct as JSON, suitable for scripting or CI integration.

## Tips

- A self-comparison (same report twice) always yields zero diff: no outcome change, zero deltas, all probes reused.
- `compare_runs` never fails — it always produces a result, even if the reports have different schema versions.
- Use run comparison in CI to detect when a code change affects regression investigation results.
