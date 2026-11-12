# Queue diagnosis review qualification — 2026-11-12

This supplements, rather than relabels, the earlier offline qualification in
[queue-diagnosis-2026-11-12.md](queue-diagnosis-2026-11-12.md).
The candidate is PR #92. It targets the reviewed contract in #88 directly;
#89 is closed as superseded because its domain implementation and necessary
correctness fixes are retained together in #92. Nothing is merged or deployed
by this receipt.

Latest qualified runtime source: **`61a33a9f477abd919436646745bef886ad3ea685`**.
The native integration/queue/CLI suite has **72 passing tests, zero failures**.
The same suite passed Windows stable and Linux Rust 1.93.0. Normal PR checks,
review-thread disposition and integration remain separate from this receipt.

## Exact code subjects

| Pass | Before | Qualified code | Run |
|---|---|---|---|
| Admission and pending evidence | `fe7d93308268279c26f4874a05ae481893af3bd1` | `2b28416ed9d5501a110260432d98681103c622a0` | [43191632848](https://github.com/EffortlessMetrics/faultline-rs/actions/runs/43191632848) |
| Equivalence-aware planning | `2b28416ed9d5501a110260432d98681103c622a0` | `95d93ab7b8f75afc0ca2ea96660e22c0f259d6eb` | [43192482101](https://github.com/EffortlessMetrics/faultline-rs/actions/runs/43192482101) |
| External review contracts | `75854e579412cd31463c31a860ea690ecaa5fd40` | `efe01ef2b2849a436fd68ada0b87797987cb9413` | [43194725033](https://github.com/EffortlessMetrics/faultline-rs/actions/runs/43194725033) |
| Ambiguous evidence reports | `d9da273fd69a81e97e6e11818443798e0dcd7549` | `61a33a9f477abd919436646745bef886ad3ea685` | [43197446532](https://github.com/EffortlessMetrics/faultline-rs/actions/runs/43197446532) |

The first qualified commit also incorporates the reviewed #88 contract at
`42d99cad3e6e80d837ad12d16fcd618cc8f2b755`. It preserves that commit as a merge
parent rather than copying the document without its ancestry. The gaps between
later qualified sources and the next inputs are documentation-only revisions
of this receipt, not unqualified runtime changes.

## Reproduced defects and corrections

1. A native `Missing` receipt previously settled its charged launch. Provider
   boundary evidence could then finalize a result while the native attempt was
   still unresolved. Missing evidence now remains pending; terminal operational
   termination and cancellation remain distinct and contribute no code vote.
2. Empty evidence, an incomplete passing sample set, or an excluded head failure
   could authorize a fresh head probe. A matching eligible target-failure sample
   at the group head is now required before proposing any new diagnostic work.
   A partial failing sample set can still request confirmation.
3. An overspent launch ledger silently saturated remaining budget to zero and
   could finalize successfully. The deduplicated historical launch total must
   now fit the declared total investigation budget. Provider-only diagnosis at
   zero budget and completed native diagnosis exactly at the limit still work.
4. An interior state below the predicate's equivalence requirement could stop
   all planning at the selected midpoint. Selection now skips unusable candidate
   states without upgrading them or deleting their transitions from the suspect
   window. Conservative scanning still refuses a first-transition claim when an
   earlier prefix is unavailable.
5. A completed native attempt whose receipt was ineligible remained pending
   forever. Completion and evidence eligibility are now separate: a structurally
   valid terminal receipt matching its charged attempt/state/commit/tree settles
   completion, stays charged and excluded, and contributes no code vote. The
   report records `launch_receipt_ineligible`. Missing output still stays pending,
   even when excluded; a different charged state/commit/tree is an integrity error.
6. Generated schemas accepted arbitrary version strings that runtime diagnosis
   rejected. Both input and report type-derived schemas now constrain the version
   to `SCHEMA_VERSION`; library and actual CLI schema tests check the constraint.
7. Markdown omitted predicate and execution-context identities. Both are now
   visible and escaped, alongside the phase and target failure signature.
8. Markdown omitted the provider/captured head for PR transitions and omitted
   commit transitions entirely. Both supported change variants now render their
   exact introduced identities, with supplied strings escaped.
9. Markdown showed pending/excluded counts but not the attempt IDs and exclusion
   reasons needed to act on them. Those identifiers and reasons are now visible
   and escaped; the existing counts remain.
10. Non-monotonic output discarded known contrary-state bounds and transitions.
    It now retains the observed interval and distinct failing-to-later-passing
    `reversals`, each with its intervening changes. Multiple segments and incomplete
    sample sets retain their evidence; Markdown exposes the segments. These are
    samples, not claims of stable recovery or independent blame.
11. An unstable interior state widened the suspect set to the whole sequence.
    Under the declared monotonic policy, tighter eligible stable bounds now define
    the window. Without that assumption, the lower bound advances only through a
    complete passing prefix. Missing earlier states remain suspects, and weak
    evidence cannot tighten the interval. Conflict never becomes exact attribution.

## Red/green evidence

The admission suite compiled against the exact pre-fix code and reported **five
specific failing regressions and four passing controls**. All nine pass with the
corrections. Compilation failure was not accepted as red evidence.

The planning suite compiled against its exact pre-fix code and reported **three
specific failing regressions and one passing control**. All four pass with the
correction. The tests exercise both monotonic binary planning and conservative
prefix scanning, including a 20-transition case that still reaches the supported
PR-14 boundary despite a weak earlier midpoint.

The first external review suite compiled against its exact pre-fix code and
reported **eight specific failing regressions and two passing controls**. All ten
pass with the corrections. The controls retain excluded-Missing blocking and
reject a receipt for another charged state. The terminal-ineligible tests cover
context, phase, snapshot and equivalence exclusion without treating them as votes.
Rendering and CLI schema tests execute the native binary, not a renderer mock.

The ambiguity suite compiled against its exact pre-fix code and reported **eight
specific failing regressions and one passing control**. All nine pass with the
corrections. Coverage includes multiple reversal segments, incomplete samples,
actual CLI rendering, tighter conflict windows, conservative incomplete/complete
prefixes and weak-equivalence exclusions. The control forbids exact attribution
from conflicting attempts.

Across this follow-up, **32 tests were added: 24 demonstrated pre-fix failures
and eight passing controls**. The final suite's 72 tests comprise 13 integration
contracts, one original monotonicity regression, nine admission contracts, five
CLI contracts, 21 diagnosis contracts, four planning contracts, ten first external
review contracts and nine ambiguity contracts. The exhaustive 20-position test
and both 128-case property tests remain; none was weakened or suppressed.

The first external qualification attempt, run 43194278329, reproduced its failures
and passed the corrected tests, then stopped on a test-helper formatting lint.
Run 43194725033 corrected that formatting and reran the complete qualification.
The first ambiguity attempt, run 43197053062, similarly stopped on a collapsible
conditional after its tests passed. Run 43197446532 applied the mechanical lint
fix and repeated red/green, strict lint, workspace and compatibility checks. The
failed attempts are not represented as final green qualification.

## Executed qualification

All four successful runs passed:

```sh
cargo test --locked -p faultline-integration -p faultline-queue --all-targets
cargo clippy --no-deps --locked -p faultline-integration -p faultline-queue --all-targets -- -D warnings
cargo xtask ci-full
cargo xtask badges
cargo xtask badges --check
cargo fmt --all -- --check
git diff --check
```

Each qualified source also passed the native integration/queue suite on:

- Linux with the declared minimum compiler, Rust **1.93.0**;
- Windows with the stable compiler selected by its run.

The Linux stable qualification records Rust 1.99.1. The workspace gate retains its
existing lint/allowance policy; strict Clippy above covers the two candidate
crates, not a claim that every older dependency is warning-free. The 58 allowance
expiries tracked by #54 were not changed.

The binary CLI tests execute the built native binary. This pass does not claim a
new registry release, macOS qualification or a live provider/worker deployment.
The earlier installed-binary smoke test remains attached to its earlier source.

## Retained evidence

Each artifact contains fresh logs, the exact input and qualified SHAs, toolchain
identity, a reviewed patch, a Git bundle and a source archive. The ZIP hashes and
contained source identifiers were independently checked after download, and the
actual patches were inspected before the review checkpoint was recorded.

| Artifact | ID | ZIP SHA-256 |
|---|---|---|
| `queue-admission-review` | `13931521458` | `21d834ae52da5f324bcf9dcbda5869c1b65bec1be2f22d4948e48f1d34774a16` |
| `queue-planning-review` | `13931681750` | `a3e2c5571561fd83d447e25c953b7bc0f6fd0ee0717219ca10d015fb1f625599` |
| `queue-external-review` | `13932716290` | `5ad1368d460ec0e2011c5899cca93beb9fc8ca2f405c44587264897ab797fb9d` |
| `queue-ambiguity-review` | `13933163581` | `dc010c6c0380478cf098db98a5c794fbe2ed0d026749093851f6d0d7fdf06fd4` |

Qualification executed with repository-read permissions. Separate publishers
checked expected parent(s), allowed file paths and branch-head leases before
fast-forwarding the existing candidate branch. All six temporary qualification
workflows were removed from the temporary branch, with final cleanup at
`62b3a61ad3e65988e35d3fd7de130a0cbb058c81`; they were never part of this candidate.
Artifact retention is finite; rerun qualification for later code rather than
reusing this receipt as current. Review replies and resolved threads are not
substitutes for an independently recorded approval.

## Remaining delivery boundary

This remains an offline diagnosis/planning component. The original execution
application still needs the immutable-attempt repair in #62 and shared identity,
launch-accounting and schema work in #63–#64. Do not attach the new planner to the
old retry path unchanged: that path currently collapses attempts into one record.

Operated dispatch additionally requires #91's real atomic reservation and crash
recovery, #59/#61's execution lifecycle and ownership, #60's complete private
logs and safe exports, #80's resolved-sequence execution, #82/#84's authenticated
capture and verified objects/prefixes, and #83's real phase receipts. #85/#86 then
cover advisory Check delivery and the live pilot. Those obligations remain open.

The duplicate runtime-test execution inside `ci_full` is recorded under #83 for
the phase-receipt refactor, with snapshot/tooling coverage and negative controls
preserved. This review did not change the CI execution graph.

Rendered Markdown is not a secret-redaction guarantee. A supplied identity is
not provider authentication; a probe proposal is not an execution reservation;
an adjacent queue trigger is not proof of an independent defect.
