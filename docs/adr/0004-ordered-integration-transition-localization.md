# ADR-0004: Diagnose integration queues as ordered transitions

## Status

Proposed.

## Context

Faultline currently localizes a regression over an ordered `RevisionSequence` derived from Git history. A merge queue presents a related but different experiment.

For an ordered group of pull requests, the meaningful candidates are cumulative integration states:

```text
S0   target-branch base
S1   base + PR 1
S2   base + PR 1 + PR 2
...
Sn   base + PR 1 ... PR n
```

When `S0` passes and `Sn` fails, the first operational question is which transition first makes the target predicate fail. The corresponding answer is a change unit such as a pull request, not merely an internal synthetic commit.

Treating the provider's synthetic commits as ordinary ancestry-path history is insufficient:

- the queue order and pull-request mapping can be lost;
- temporary refs can be rebuilt or deleted before diagnosis starts;
- locally reconstructed states can differ from the provider's actual state;
- a red check can represent a code failure, a different code failure, cancellation, timeout, missing status or infrastructure failure;
- an adjacent pass/fail transition proves a trigger in that ordering, not necessarily an independent defect in the introduced pull request.

Issue #58 separately records that a branched ancestry path can contain incomparable sibling commits. Queue support must not rely on another implicit graph linearization.

## Decision

Faultline will diagnose merge queues as externally resolved ordered integration sequences.

### Sequence resolution and localization are separate

The application layer will expose one use case that accepts a validated immutable candidate sequence and eligible seed evidence. Existing Git-history investigations will resolve history and delegate to that use case. Provider adapters will construct integration sequences and delegate to the same use case.

The pure binary-narrowing engine remains provider-neutral.

### Integration transitions are first-class domain objects

A pure integration-domain model will retain:

- state ordinal, commit and tree identity;
- cumulative included change units;
- the single transition unit introduced at each non-base state;
- state equivalence and provenance;
- queue-facing outcomes that map localization boundaries to transitions.

An adjacent boundary `S13 pass -> S14 fail` maps to the transition that introduced PR #14.

### Provider evidence is captured before failure diagnosis

A GitHub adapter will capture `merge_group: checks_requested`, ordered queue membership and the synthetic Git object closure immediately. A terminal `destroyed` event appends lifecycle state without deleting the incident.

Capture is authenticated, idempotent and read-only. The control plane does not execute pull-request code.

### Reconstructed states carry explicit equivalence

Provider-generated states are preferred. Missing prefixes may be reconstructed, but every state records one of:

- `ExactCommit`;
- `ExactTree`;
- `PatchEquivalent`;
- `Unverified`.

A predicate declares the minimum equivalence it requires. A weaker reconstruction cannot support a stronger attribution claim.

### One stable failure is localized at a time

CI emits machine-readable phase receipts and stable failure signatures. Cancellation, timeout, missing status and unavailable runners are not code failures. A different failure at an intermediate state is not treated as the target failure.

Imported provider observations and native Faultline probes remain distinct evidence sources and are reused only when state, predicate, classification protocol and declared execution context are compatible.

### Queue results describe transitions

Queue-facing outcomes are:

- exact first failing transition;
- suspect transition window;
- non-monotonic evidence;
- inconclusive.

A suspect window includes every supported transition in the window. It is never collapsed to one blamed pull request.

### Trigger and causal attribution remain distinct

The first result states that a pull request triggers the failure in the observed ordering. Stronger classifications—independent regression, interaction, dependency-constrained result or multiple regressions—require explicit counterfactual evidence.

### Diagnosis is advisory

The GitHub result is a non-required `Faultline / queue diagnosis` Check on the exact merge-group head. The first version does not dequeue, merge, requeue, label or modify pull requests automatically.

## Consequences

- A deterministic 20-transition sequence can normally be narrowed with at most five fresh midpoint probes after eligible endpoints are available.
- Healthy queues incur no Faultline probing.
- The application gains a provider-neutral resolved-sequence seam.
- Queue capture, state construction, failure classification and localization remain separately testable.
- Reports become more explicit about identity, equivalence, evidence source and claim strength.
- The work depends on the existing executor and evidence hardening in issues #58-#64 and #55.
- Counterfactual interaction search follows the first useful transition result rather than delaying it.

## Non-goals

This decision does not make Faultline:

- a merge-queue controller;
- a required queue gate;
- a general Git DAG-frontier solver;
- a second CI scheduler;
- an authority that automatically removes or lands pull requests.

## Work map

The implementation campaign is tracked in #87. Queue-specific slices are #80-#86.