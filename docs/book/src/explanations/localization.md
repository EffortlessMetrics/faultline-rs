# How Localization Works

This page explains the core algorithm that faultline uses to narrow a regression window: binary narrowing with honest outcome classification and confidence scoring.

## The problem

Given a linear sequence of Git commits between a known-good and known-bad revision, faultline needs to find where the regression was introduced. Each commit can be probed by running the operator's predicate (a test or build command), but probing is expensive — it requires checking out the revision and executing the command. The goal is to minimize the number of probes while producing an honest result.

## Binary narrowing

Faultline uses a binary search strategy adapted for the realities of real-world Git history, where commits can be untestable, predicates can time out, and evidence can be non-monotonic.

### Boundary probing

The algorithm starts by probing the two boundary commits:

1. **First commit** in the sequence (expected to pass)
2. **Last commit** in the sequence (expected to fail)

These boundary probes establish the initial pass/fail pair. If either boundary hasn't been probed yet, it takes priority over any interior probing.

### Midpoint selection

Once boundaries are established, the algorithm finds the tightest pass/fail boundary pair — the highest-index passing commit and the lowest-index failing commit above it. It then collects all unobserved commits between these boundaries and selects the **median** as the next probe target.

This is classic binary search: each probe halves the remaining candidate window. For a sequence of N commits, the algorithm converges in O(log N) probes in the ideal case.

### Termination conditions

The algorithm stops probing when any of these conditions is met:

- The pass/fail boundary is adjacent (no unobserved commits between them) — the regression is pinpointed
- The maximum probe budget (`max_probes`) is exhausted
- No unobserved candidates remain between the boundaries

## Outcome classification

After probing completes, faultline classifies the result into one of three outcome types. This classification is the core of faultline's "honest over impressive" design principle — it never claims more precision than the evidence supports.

### FirstBad

The ideal outcome. The algorithm found an adjacent pass/fail pair with no ambiguity between them:

- The commit at the lower index passed
- The commit at the upper index failed
- No skipped, indeterminate, or unobserved commits exist between them

This means the upper commit is the exact first-bad commit. Confidence is **high** (score 90).

### SuspectWindow

The algorithm found a pass/fail boundary, but there is ambiguity within the window. Reasons include:

- **SkippedRevision** — a commit between the boundaries returned exit code 125 (untestable)
- **IndeterminateRevision** — a commit between the boundaries timed out or produced an ambiguous result
- **NonMonotonicEvidence** — a failing commit appears at a lower index than a passing commit, suggesting the regression is not a clean transition

When non-monotonic evidence is present, confidence drops to **low** (score 30). Otherwise, suspect windows get **medium** confidence (score 60).

### Inconclusive

The algorithm could not establish a meaningful boundary. Reasons include:

- **MissingPassBoundary** — no commit was observed to pass
- **MissingFailBoundary** — no commit was observed to fail
- **NeedsMoreProbes** — unobserved commits remain between the boundaries but the probe budget is exhausted
- **MaxProbesExhausted** — the maximum number of probes was reached before convergence

An Inconclusive result means the operator should either increase the probe budget, improve the predicate's reliability, or manually investigate the commit range.

## Confidence scoring

Every outcome carries a `Confidence` value with a numeric `score` (0–100) and a human-readable `label`:

| Level | Score | Label | When |
|-------|-------|-------|------|
| High | 90 | `high` | Adjacent pass/fail pair, no ambiguity |
| Medium | 60 | `medium` | Suspect window with skipped or indeterminate commits, but monotonic evidence |
| Low | 30 | `low` | Non-monotonic evidence detected |

Confidence is a property of the evidence, not a probability. A high-confidence result means the evidence cleanly supports the conclusion. A low-confidence result means the evidence is contradictory and the window should be treated as a starting point for manual investigation.

## Ambiguity reasons

Each non-FirstBad outcome includes a list of `AmbiguityReason` values explaining why the result is not exact:

| Reason | Meaning |
|--------|---------|
| `MissingPassBoundary` | No passing observation was recorded |
| `MissingFailBoundary` | No failing observation was recorded |
| `NonMonotonicEvidence` | A fail was observed before a pass in sequence order |
| `SkippedRevision` | A commit returned exit 125 (untestable) |
| `IndeterminateRevision` | A commit timed out or was operationally ambiguous |
| `UntestableWindow` | The entire window is untestable |
| `BoundaryValidationFailed` | The good/bad boundaries did not validate as expected |
| `NeedsMoreProbes` | Unobserved commits remain in the window |
| `MaxProbesExhausted` | The probe budget was fully consumed |

Multiple reasons can apply simultaneously. For example, a result might be `Inconclusive` with both `NeedsMoreProbes` and `MaxProbesExhausted` if the budget ran out before the window could be fully explored.

## Putting it together

A typical localization session proceeds as:

1. Build the revision sequence from `git rev-list` between good and bad
2. Probe the first commit → expect Pass
3. Probe the last commit → expect Fail
4. Binary-narrow: pick the median unobserved commit between the tightest pass/fail pair
5. Record the observation and repeat from step 4
6. When termination is reached, classify the outcome and compute confidence
7. Emit the `AnalysisReport` with all observations, the outcome, and the confidence score

The result is always honest: if the evidence is clean, you get `FirstBad` with high confidence. If it's messy, you get a `SuspectWindow` or `Inconclusive` with the reasons why, so you know exactly where to look next.
