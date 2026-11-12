# Merge-queue localization

## Product contract

When an exact cumulative merge group goes red, Faultline should preserve the experiment, identify one reproducible failure, localize the first transition where that failure appears, map the transition to the queued pull request, and state only the causal strength supported by the evidence.

For an ordered group of 20 pull requests:

```text
S0   target-branch base
S1   base + PR 1
S2   base + PR 1 + PR 2
...
S20  base + PR 1 ... PR 20
```

With eligible evidence that `S0` passes and `S20` fails, there are 20 possible failing transitions. Sequential binary narrowing needs at most five fresh midpoint probes because `2^5 > 20`.

The search is small. The product difficulty is preserving the correct states, reproducing the same failure, executing probes safely and distinguishing a queue-order trigger from stronger causal attribution.

## Required result

An exact first-transition report should read conceptually as:

```text
Target failure:
  CI Full / workspace-tests / schema_round_trip

Last passing integration state:
  S13 — base + PRs 1-13

First failing integration state:
  S14 — base + PRs 1-14

Transition:
  PR #14 at exact head 8d3a...

Established:
  Adding PR #14 to this cumulative state triggers the target failure.

Not established:
  PR #14 fails independently.
  No earlier PR contributes to the failure.
  The change did not expose a latent defect.
```

A suspect result names every transition in the supported interval. It does not select one pull request for convenience.

## Terms

**Queue incident**
: One immutable capture of a provider merge-group event, queue snapshot and retained Git object closure.

**Integration state**
: One cumulative candidate state in the ordered experiment.

**Transition unit**
: The single change unit introduced between adjacent states. The initial provider implementation uses pull requests.

**Failure track**
: One stable target failure identity and its compatible predicate, execution context and observations.

**Provider observation**
: Evidence imported from CI or another integration provider.

**Native attempt**
: A predicate launch performed by Faultline's worker.

**State equivalence**
: The proven relationship between a candidate state and the provider state it represents.

## System boundaries

```text
GitHub merge-group event
          |
          v
+------------------------+
| provider control plane |
| authenticate           |
| snapshot queue         |
| retain Git objects     |
| observe CI             |
+-----------+------------+
            |
            v
+------------------------+
| queue forensics        |
| validate order         |
| build prefix states    |
| grade equivalence      |
| define failure tracks  |
+-----------+------------+
            |
            v
+------------------------+
| Faultline application  |
| ingest resolved states |
| seed eligible evidence |
| select and run probes  |
| localize boundary      |
+-----------+------------+
            |
            v
+------------------------+
| integration result     |
| map state to PR        |
| render evidence        |
| publish neutral Check  |
+------------------------+
```

The provider control plane does not execute pull-request code. The worker does not receive GitHub App credentials or publication secrets.

## Sequence resolution

The application must not rediscover every investigation through `HistoryPort::linearize`.

The target application surface is:

```text
history intent
  -> resolve valid Git history
  -> localize resolved sequence

queue incident
  -> resolve valid integration sequence
  -> localize resolved sequence
```

The existing Git-history CLI remains supported. It resolves history and delegates to the same resolved-sequence use case used by integration adapters.

A resolved sequence contains immutable candidate identities and enough metadata to map a boundary back to its source. Provider-specific network behavior stays outside the localization crate.

## Integration sequence invariants

An integration sequence is valid only when:

1. it contains at least a base and one transition;
2. ordinals are contiguous and begin at zero;
3. candidate state and commit identities are unique;
4. the base has no transition unit;
5. every later state has exactly one transition unit;
6. the cumulative included set at state `n` equals state `n-1` plus that transition;
7. ordering, removals or replacements are not hidden;
8. every state declares provenance and equivalence;
9. a result boundary can be mapped to adjacent state indices.

Invalid sequence structure fails before a probe runs.

## State equivalence

Provider-generated states are preferred. Locally generated states remain useful only when their relationship to the provider experiment is explicit.

| Equivalence | Meaning | Typical eligible predicate |
|---|---|---|
| `ExactCommit` | Same commit and tree | History-, release- or metadata-sensitive checks |
| `ExactTree` | Same tree, different commit metadata/history | Ordinary build, lint and test checks that depend on repository bytes |
| `PatchEquivalent` | Bounded patch equivalence only | Investigation guidance; not high-confidence attribution by default |
| `Unverified` | Relationship not established | No exact boundary claim |

Each predicate declares the minimum state equivalence it requires. A weaker state cannot establish a stronger claim.

## GitHub incident capture

Capture begins on `merge_group: checks_requested`, before CI failure is known.

The adapter:

1. validates the webhook signature;
2. deduplicates the delivery;
3. stores the raw event privately with a digest;
4. records merge-group head/base commit, refs, tree and event time;
5. queries the ordered queue entries and queue configuration;
6. records PR numbers, positions, exact heads, entry commits and state flags;
7. fetches and retains the synthetic ref and required object closure;
8. verifies retained commit/tree identity against the event;
9. creates a provisional provider-neutral sequence mapping;
10. appends later `destroyed` lifecycle events without deleting evidence.

A queue jump, base movement, PR-head movement or rebuilt provider head creates a distinct incident revision. A race that cannot reconcile membership to the event head is explicit ambiguity, not a guessed order.

## Prefix-state construction

Candidate states are obtained in this order:

1. exact provider-generated prefix states;
2. exact provider observations attached to those states;
3. locally reconstructed prefixes from the exact base and ordered PR heads.

For reconstruction, record:

- merge method and strategy;
- Git version;
- exact input commits and trees;
- conflict handling;
- generated commit and tree;
- final comparison to the retained provider head.

If the reconstructed final tree differs from the provider tree, the reconstructed sequence cannot support exact queue attribution.

A conflict, missing object or dependency-constrained state becomes skipped or unsupported. It is not silently removed from the sequence.

## Failure tracks

A red merge group is not itself a predicate.

The controller separates:

- reproducible code failure;
- different code failure;
- cancellation;
- timeout;
- runner or external-service failure;
- missing required status;
- unavailable or incompatible evidence.

Only a reproducible code failure enters ordinary localization.

Each failure track contains:

```text
check and phase identity
normalized predicate
classification protocol
execution-context identity
target failure signature
provider observation
claim limitations
```

Useful failure signatures include:

- test class and test name;
- compiler diagnostic code and primary span;
- clippy lint and path;
- snapshot identity;
- schema or policy rule identity;
- normalized output digest as a bounded fallback.

At an intermediate state:

```text
target signature present                    => Fail
target signature absent and predicate passes => Pass
different failure present                   => DifferentFailure
timeout or unavailable execution            => Indeterminate
```

This prevents one regression from being localized using evidence from another.

## CI receipts

Mandatory CI emits a versioned receipt for every meaningful phase. A receipt contains:

```text
exact commit and tree
workflow, job and step identity
normalized command and working directory
timeout and classification protocol
declared runner/toolchain/environment identity
result and termination metadata
stable failure signatures
retained artifact references and digests
limitations and claim boundary
```

`cargo xtask ci-full` remains the mandatory aggregate gate. Faultline should normally probe the narrow failing phase or case rather than rerun the entire gate at every midpoint.

Provider receipts can seed localization only when candidate state, predicate, protocol and declared execution context match the resolved investigation.

## Localization

The application loads:

- the validated integration sequence;
- one failure track;
- eligible provider observations;
- eligible prior native evidence.

It validates endpoints, then performs binary narrowing over the ordered candidate IDs.

For 20 transitions, an example path to PR 14 is:

```text
S0 pass, S20 fail
S10 pass
S15 fail
S13 pass
S14 fail
```

The result is exact only when:

- the pass and fail states are adjacent;
- both observations are eligible and stable;
- the target failure signature agrees;
- state equivalence meets the predicate requirement;
- the sequence maps the transition to exactly one change unit;
- no relevant skip, indeterminate or non-monotonic evidence remains.

Otherwise Faultline returns the narrowest supported suspect set or an inconclusive result.

## Non-monotonic evidence

The target failure can appear, disappear and reappear:

```text
P P F F P P F F
```

When contradictory evidence appears, Faultline stops claiming one first-bad boundary under the monotonic model. Queue mode preserves observed transitions and may use a bounded scan to map them.

Failure signatures reduce accidental non-monotonicity caused by unrelated failures, but do not eliminate genuine repair-and-regression sequences.

## Search policy and cost

Sequential binary narrowing minimizes launches. A later bounded parallel strategy may spend more launches to reduce wall-clock rounds.

Every budget counts actual predicate launches, including:

- endpoint validation;
- midpoint probes;
- retries;
- speculative parallel probes;
- counterfactual attribution probes.

Cache reuse consumes no launch budget but records why the evidence was accepted.

Healthy queues run no Faultline probes.

## Queue-facing outcomes

```text
FirstFailingTransition
  last passing state
  first failing state
  transition unit
  state equivalence
  confidence

SuspectTransitions
  lower passing bound
  upper failing bound
  every transition in the interval
  ambiguity reasons

NonMonotonic
  observed pass/fail transitions
  target signature
  ambiguity reasons

Inconclusive
  missing or ineligible boundaries
  operational/evidence reasons
```

The existing `LocalizationOutcome` remains the underlying search result. Integration-domain logic converts it using the validated sequence metadata.

## Trigger versus attribution

An adjacent transition establishes a trigger in this ordering.

Stronger attribution requires counterfactual evidence:

1. full group without the trigger PR;
2. trigger PR on the exact base;
3. trigger PR with minimum declared dependencies;
4. selected earlier PR combinations based on overlapping paths, manifests, lockfiles, feature flags and the target signature;
5. dependency-constrained delta debugging only where subsets remain meaningful.

Possible classifications are:

- trigger in queue order;
- independent regression supported;
- interaction supported;
- dependency constrained;
- multiple regressions;
- inconclusive.

The first useful transition result does not wait for exhaustive interaction search.

## Storage

```text
queue-incidents/<incident-id>/
  event/
  snapshot/
    queue.json
    merge-group.json
    sequence.json
    objects.bundle
    object-manifest.json
  failures/<failure-track-id>/
    predicate.json
    imported-observations/
    attempts/<attempt-id>/
      attempt.json
      stdout.raw
      stderr.raw
    aggregates/
    report.json
    export/
      manifest.json
      analysis.json
      index.html
      dossier.md
```

Events and attempts are append-only. Aggregates can be recomputed. Raw logs remain private. Shareable artifacts are explicit redacted projections into a fresh owned root.

## GitHub output

Publish a non-required Check on the exact merge-group head:

```text
Faultline / queue diagnosis
```

Use:

- `neutral` when an exact, suspect or honest inconclusive diagnosis completes;
- `cancelled` when the incident is invalidated;
- `failure` only when Faultline itself fails.

The Check states:

- target failure;
- queue size and incident identity;
- last passing and first failing state or suspect window;
- implicated transition unit where established;
- exact versus reconstructed equivalence;
- provider observations, cache reuse and fresh launch counts;
- trigger-versus-attribution boundary;
- likely changed surfaces;
- retained evidence link and limitations.

The first version does not automatically mutate the queue or pull requests.

## Safety

- The control plane validates events and captures evidence; it does not run untrusted code.
- The worker receives no GitHub App or publication credentials.
- Repository locks, worktrees and recovery use the ownership model from #61.
- Timeout and cancellation terminate the complete owned process tree under #59.
- Immutable attempts and honest retry aggregation use #62.
- Complete private logs and safe exports use #60.
- Reuse identity uses #63.
- Actual launch budgets and installed behavior use #64 and #55.

## Acceptance pilot

The controlled pilot covers:

- 20 PRs with deterministic failures at transitions 1, 14 and 20;
- multiple commits inside one PR;
- a two-PR interaction where both pass alone;
- a later repair producing non-monotonic evidence;
- two failure signatures at different transitions;
- flaky endpoints and interior states;
- merge, squash and rebase strategies;
- provider prefix observations and head-only evidence;
- queue reorder, jump, base movement, PR-head movement and invalidation;
- temporary-ref deletion after successful capture;
- exact-tree reconstruction match and mismatch;
- interruption, process descendants and stale-worktree recovery;
- synthetic-secret output and export leak tests.

For a monotonic deterministic target across 20 valid transitions, sequential mode uses no more than five fresh midpoint launches after eligible endpoints are available.

## Work map

The complete campaign is #87.

- #80 — resolved-sequence application seam
- #81 — integration sequence and transition domain
- #82 — GitHub incident capture
- #83 — CI receipts and failure signatures
- #84 — prefix-state construction and equivalence
- #85 — diagnosis and Check publication
- #86 — causal refinement and controlled pilot

Existing executor/evidence prerequisites are #58-#64 and #55.