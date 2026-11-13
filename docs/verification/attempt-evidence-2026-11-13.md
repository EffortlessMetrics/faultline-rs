# Probe-attempt evidence qualification — 2026-11-13

Candidate: PR #93, stacked on #92. This receipt covers completed-attempt
preservation and retry aggregation in the history execution application, its
FileRunStore adapter, report exports and CLI mode wiring. It is not live queue
dispatch, atomic admission, a registry release or a merge decision.

## Exact subjects

- Initial qualified producer: `3cab51d0f5a71dd5e6938e3deef0294eaee30eb4`.
- Queue runtime parent: `d168c05992eca029df5644a38247f1428e2744db`.
- Reconciled queue parent: `3c2f876e631e678dc670b8f424149861fe1ae51d`.
- Final qualified source: **`20221522d56e5210738ce04b989117e5f5630fa4`**.
- [Initial producer qualification, run 43405894866](https://github.com/EffortlessMetrics/faultline-rs/actions/runs/43405894866).
- [Final source qualification, run 43408812406](https://github.com/EffortlessMetrics/faultline-rs/actions/runs/43408812406).

The final source restores existing Rust line endings, corrects two scenario
index descriptions, incorporates the queue parent's receipt and refreshes the
source-sensitive badge. Its normalized Rust bytes are identical to the initial
qualified producer. That comparison was checked in the qualification job and
again against both downloaded source archives. The final diff against its queue
parent is 24 files, 1,280 additions and 168 deletions before this receipt.

## Behavior verified

The application retains each returned raw execution before cleanup, another
retry or aggregation. Separate aggregates link immutable run-local attempt IDs.
Multi-attempt search projections do not borrow a first attempt's exit status,
output, duration or working directory. Mixed results are indeterminate even
when the legacy stability threshold would permit a majority.

The producer suite contains **15 tests**, including a property test configured
for **128 generated cases**, covering:

- persistent store reopening with original pass/fail/pass metadata intact;
- aggregate metadata separation and rejection of invalid raw inputs;
- completed-batch resume with no new execution;
- retention across a later retry error and cleanup failure;
- idempotent writes, rejection of rewritten IDs and preservation through cache clearing;
- corrupt records and tampered/legacy evidence-free caches failing closed;
- force preserving earlier attempts while creating new attempt identities;
- unstable endpoints producing inconclusive output;
- redaction of new attempt fields across report exports;
- captured truncation markers not advertising unavailable complete logs;
- generated mixed histories never becoming decisive and never altering originals.

A separate **real Git repository and native CLI** test uses a deterministic
`git ls-files --error-unmatch PASS` predicate. With two retries it observes nine
original executions, an identical resumed journal, eighteen retained executions
after force, and a new nine-attempt journal starting at zero after explicit fresh.
This caught and repairs the previous failure to forward parsed mode flags into
`LocalizeOptions`.

This invokes the Cargo-built native executable. It is **not** a task-isolated
`cargo install` proof, nor does it establish every destructive-cleanup sentinel
condition in #55/#61. Those acceptance obligations remain open.

## Falsification

Two deliberate mutations were executed and rejected:

1. Removing the raw journal write fails the reopen/persistence regression.
2. Making a mixed aggregate decisive fails the aggregation regression.

Restoring each implementation returns the tests to green. These are deliberate
mutation controls, not claims that newly introduced APIs compiled on the old
source. The separate queue identifier test did compile against its actual old
implementation and fail before the correction; its receipt is
[queue-identities-2026-11-13.md](queue-identities-2026-11-13.md).

## Executed qualification

The initial and final sources passed:

```sh
cargo test --locked --workspace --all-targets
cargo xtask ci-full
cargo xtask badges
cargo xtask badges --check
cargo fmt --all -- --check
```

The explicit workspace test invocation reports **589 passed, zero failed**.
The canonical full gate also verifies policy, snapshots, schema consistency and
the scenario atlas. Existing workspace lint/allowance policy remains unchanged;
this is not a claim of warning-free dependencies or renewed allowances.

Initial compatibility ran application/store/queue/integration tests and the real
CLI test on Windows stable and Linux Rust 1.93.0. Final compatibility repeated the
producer persistence suite and real CLI mode test on both platforms. Linux stable
qualification recorded `rustc 1.100.0 (0ef1bc386 2026-11-09)`.

Existing tracked CRLF source files were preserved. The final whitespace check
recognizes CRLF endings while retaining trailing-blank, blank-at-EOF and
space-before-tab checks:

```sh
git -c core.whitespace=blank-at-eol,blank-at-eof,space-before-tab,cr-at-eol diff --check
```

This is a qualification-command setting, not a committed relaxation of repository
policy. Earlier mechanical-cleanup attempts stopped on a stale badge and on Git
interpreting restored CRLF characters as trailing whitespace. They are not
represented as successful final runs. The final run regenerated and checked the
badge and completed the appropriate whitespace check.

## Retained artifacts

| Artifact | Run / artifact ID | ZIP SHA-256 |
|---|---|---|
| `producer-attempt-proof` | 43405894866 / 14026969237 | `0b31cf89efbd4234daebcdb7981ffc3332df8845a7bb38054177eb8817ce2226` |
| `final-attempt-proof` | 43408812406 / 14027963862 | `c99bc9c2e0e0ad720ac445018f92ff908f0021b73f8bade1ff870d39758cb1b1` |

Both downloaded ZIP hashes were checked. The source, patches, qualified SHAs,
commands and test logs were inspected. The final archive includes the source
bundle, source tar, diff/stat, line-ending audit and toolchain. Artifact retention
is finite; this durable receipt does not substitute for requalification of later
code. Normal final-head PR gates are recorded separately after this receipt lands.

Qualification jobs used repository-read permission. Separate publishers checked
expected parent(s), allowed paths and branch-head leases before advancing only
the candidate branches. No temporary qualification workflow is in either PR.

## Remaining work

New history reports emit AnalysisReport schema **0.4.0** with optional/defaulted
attempt and aggregate fields. Older reports remain readable without manufacturing
missing originals. The queue diagnosis schema remains **0.1.0**.

#93 does not implement durable prelaunch/start/nonterminal records, a total
actual-attempt budget, atomic concurrent admission, full context-bound reuse,
power-loss recovery, complete untruncated logs or filesystem containment. A
partial journal record fails closed. A process error without a returned result
still needs explicit lifecycle evidence. #59–#64/#91 retain those obligations.
Run comparison and the remaining #62 migration acceptance also remain open.

The 58 no-panic allowance expiries under #54 were not changed. No deployment,
queue/PR automation, required-check cutover or release was performed.
