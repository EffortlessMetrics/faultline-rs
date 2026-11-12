# Offline queue diagnosis

`faultline-queue` is a native Rust replay and planning tool for captured cumulative
integration states. It returns a transition, a suspect window, a refusal, or the
next exact state whose evidence is needed. It never executes supplied commands,
fetches Git objects, contacts GitHub, or changes a queue.

This is an executable slice of campaign #87, not the live collector/worker/Check
integration. Provider authenticity and object custody must be established before
real receipts enter this tool. Internal identity consistency is not authentication.

## Install and try the synthetic example

From the reviewed workspace candidate:

```sh
cargo install --path crates/faultline-queue --locked
faultline-queue --help
faultline-queue demo > synthetic-incident.json
faultline-queue diagnose --input synthetic-incident.json --output plan.json
```

The final command deliberately exits **2**: it needs evidence, not a green check.
With the two synthetic provider endpoint receipts, it proposes S10. There are no
native launches and there are no real Git objects or logs behind this demo.

For an operator-readable view:

```sh
faultline-queue diagnose --input synthetic-incident.json --format markdown
```

Schemas come from the actual Rust input and output types:

```sh
faultline-queue schema input > queue-diagnosis-input.schema.json
faultline-queue schema report > queue-diagnosis-report.schema.json
```

Input files are limited to 16 MiB. Output files use create-new semantics: a
previous report, an input file, or a symlink is never silently overwritten.
Standard output is available for pipelines. Exit **3** means invalid input or an
I/O failure; **0** means a supported transition or a passing head for this target;
all unresolved diagnostic statuses exit **2**. None of these is merge authority.

## Input contract

`schema_version` is `0.1.0`. A packet contains:

- `sequence`: one validated `IntegrationSequence`, S0 through Sn, with immutable
  commit and tree identities, cumulative membership, source snapshot and the
  exact PR/head introduced by each transition;
- `target`: phase, stable failure signature, predicate identity and execution
  context identity;
- `policy`: total native-launch budget, minimum matching attempts, explicit
  monotonicity policy, and minimum state equivalence;
- `launches`: the durable native launch ledger, including attempts whose output
  is still missing;
- `receipts`: immutable phase outcomes from the provider or native workers.

Obtain the complete shape from `schema input` or `demo`; do not infer it from a
human log summary. The collector must normalize receipts into this shape. The
Python authenticated incident in PR #90 is not yet automatically converted into
this packet. No collector is implied by accepting JSON.

A failure receipt requires a nonzero exit code and a nonempty signature. A pass
requires exit zero and no failure signature. Operational, cancelled and missing
results cannot carry a code-failure signature. Other failed tests are **not**
passing evidence for the target and are not interchangeable with its signature.

Every eligible receipt matches snapshot, state ID, commit, tree, phase, predicate
and context. Rejected receipts remain listed with a reason. A native receipt must
match an already-charged launch. Relabeling a native launch as provider evidence
is rejected. Identical duplicate records are idempotent; conflicting records
under one attempt ID are invalid. Authenticity and independent-attempt identity
must be enforced by the upstream collector, not by freely supplied ID strings.

## What can be claimed

The conservative policy (`assume_monotonic: false`) needs observed passing
evidence for **every earlier state** before calling a transition the first. It
plans those missing states in order.

With `assume_monotonic: true`, the report records `MonotonicTargetAssumed` and
reuses the existing localization engine for binary midpoint selection. The
assumption means this particular failure persists once introduced. It is not a
proof that arbitrary code histories are monotonic. Observed fail-then-pass
evidence stops binary planning, including contrary samples that have not yet
met the configured repeated-sample threshold.

A full earlier passing prefix records `CompletePrefixObserved`. An unstated
basis never becomes an exact first-transition claim. First-transition mapping
requires at least exact-tree equivalence, even when a caller asks for a weaker
minimum. Reconstructed exact-tree states need a retained witness for the **same
prefix**. Matching only the final group tree proves no intermediate prefix.

Pass/fail conflicts remain unstable; there is no majority vote and later attempts
do not overwrite earlier ones. Operational attempts consume launch budget but do
not become code votes. Subsequent coherent decisive evidence can still establish
a state after an operational interruption. A pending launch prevents both another
proposal and finalization until its matching evidence arrives.

Even a supported first transition is a trigger in this queue ordering. It does
not establish an independent defect or rule out interaction with an earlier PR.

## Cost and continuation

The pure diagnosis call performs **zero launches**. `next_probe` proposes at most
one new launch and includes the exact state, target and source snapshot. The
external executor must durably reserve/charge that launch before starting work,
then retain the resulting receipt and logs. Re-running the same input produces
the same proposal; replay is not an exactly-once dispatcher or a reservation.
Do not let two workers independently execute the same plan.

Provider evidence costs no new native launches. Every native ledger entry does,
including retries and missing receipts. The legacy search engine's count of
cached observations is deliberately not used as the launch budget. A completed
result may be replayed at budget zero. A budget-limited result retains its supported
suspect window instead of discarding it or naming one PR.

For 20 deterministic transitions with eligible provider endpoints, one matching
attempt per tested state, and the explicit persistence assumption, the exhaustive
test covers **all 20 failure positions** with at most five native midpoint
launches. Requiring two matching attempts can require ten midpoint launches.
These are synthetic algorithm/contract tests, not a live merge-queue pilot.

## Architecture

`faultline-integration` owns pure state identities, provenance consistency,
equivalence and transition mapping. `faultline-queue` is the application layer:
it validates and aggregates captured phase receipts and calls
`faultline-localization` for binary planning. Its binary is a thin file/JSON/CLI
adapter. No provider SDK, shell, Git runner or credentials enter either library.

## Remaining operated path

The following obligations remain open and must not be inferred from this CLI:

1. #82: authenticated live capture, exact membership/order, retained Git objects,
   bundle verification and the captured-incident-to-native-packet adapter.
2. #83: actual CI phase emitters, stable signature normalization and context
   fingerprints that reflect toolchain, dependencies, workflow and environment.
3. #58–#64, #80, #84: resolved-sequence execution, isolated owned worktrees,
   process-tree limits, atomic reservations, immutable complete logs, state
   construction and interrupted-worker reconciliation.
4. #85–#86: event-driven coordinator, non-required Check publication, bounded
   counterfactual work and the controlled **live** 20-PR qualification.

Neither a schema-valid packet, the synthetic test suite, nor a repository badge
closes these obligations. There is no automatic dequeue, requeue, merge or PR edit.
