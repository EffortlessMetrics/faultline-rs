# ADR-0003: Honest localization outcomes

## Status
Accepted.

## Context

Traditional bisect tools (e.g., `git bisect`) assume monotonic predicate behavior and return a single "first bad commit" even when the evidence is ambiguous. In practice, predicates can flake, time out, or depend on mutable external state, making a single-commit answer misleading.

We need an outcome model that communicates the actual confidence level supported by the evidence.

## Decision

The localization engine returns one of three outcome types:

- **FirstBad** — exact first-bad commit identified, with the adjacent last-good commit. Requires direct Pass and Fail observations on the boundary pair. Assigned high confidence.
- **SuspectWindow** — a range of commits that likely contains the regression, but the evidence is ambiguous. Includes specific `AmbiguityReason` tags (SkippedRevision, IndeterminateRevision, NonMonotonicEvidence) and a reduced confidence score.
- **Inconclusive** — insufficient evidence to narrow the window at all (e.g., no pass boundary established, no fail boundary established).

The engine never claims more certainty than the evidence supports.

## Consequences

- Operators get honest feedback about result quality instead of false precision
- Downstream tooling can branch on outcome type (e.g., auto-file a bug only for FirstBad)
- Non-monotonic histories produce suspect windows with explanatory reasons rather than wrong answers
- The confidence score and ambiguity reasons are included in both JSON and HTML artifacts
