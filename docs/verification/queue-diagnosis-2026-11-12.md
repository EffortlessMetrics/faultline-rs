# Native queue diagnosis qualification — 2026-11-12

Scope: PR [#92](https://github.com/EffortlessMetrics/faultline-rs/pull/92),
stacked on #89. This receipt covers the offline evidence/diagnosis CLI, not live
GitHub capture, predicate execution, Check publication or production deployment.

## Exact subject and run

- Qualified code commit: `ba20f749bbc26be9e25a687489701069649cca89`.
- Qualification input: `4990cbf070bbb40e929a0dd72e4d6b6172249f2a`.
- [Hosted qualification run 43187810125](https://github.com/EffortlessMetrics/faultline-rs/actions/runs/43187810125).
- Toolchain: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`,
  `x86_64-unknown-linux-gnu`, LLVM 23.1.1.
- Evidence artifact: `queue-qualification-evidence`, ID `13930250132`,
  ZIP SHA-256 `7cfa914e18335d38aa077a6bd1f709ab17d54e61783c461257b91c0ce4899079`.
- Qualified commit bundle: artifact ID `13929970664`, ZIP SHA-256
  `72f07e4b14c1bf7666dfe8bdd07572b33e2e840475fae0329b7b89f07faf608f`.

The qualification job ran with repository-read permissions. A separate publisher
verified the bundle's parent and changed-file scope, required the source-branch
lease, and fast-forwarded only the qualified commit. The branch-specific
bootstrap workflow was deleted before workspace qualification and is absent
from the qualified tree. No permanent writer workflow is introduced.

Evidence output was cleared before the run. The artifact includes the input
SHA, qualified SHA, toolchain, red/green logs, scoped Clippy log, workspace log,
generated input/report schemas, synthetic input and the resulting plan. The ZIP
digest and its contained SHAs were independently checked after download.
Artifacts have finite retention; the source, test definitions and this receipt
remain in Git. Re-run qualification rather than treating an expired artifact or
an earlier source SHA as current evidence.

## Executed checks

```sh
cargo test -p faultline-integration -p faultline-queue --all-targets
cargo clippy --no-deps -p faultline-integration -p faultline-queue --all-targets -- -D warnings
cargo xtask ci-full
cargo xtask badges
cargo xtask badges --check
cargo fmt --all -- --check
git diff --check
cargo install --path crates/faultline-queue --locked --debug --root target/queue-install
```

All completed successfully. The native test command reported **40 passing tests,
zero failures**: 13 integration-contract tests, one standalone regression test,
five binary CLI tests and 21 diagnosis tests. The two property tests each request
128 cases; these are generated test cases, not 256 independent live incidents.

The workspace gate ran under the repository's existing lint policy. Strict
Clippy above is scoped to the two candidate crates; it is not a claim that every
existing dependency is warning-free. Existing no-panic allowance work remains
[#54](https://github.com/EffortlessMetrics/faultline-rs/issues/54), with its original
expiry and scope unchanged.

## Falsification and cost evidence

`default_mapping_never_assumes_monotonicity` was compiled and run against the
old domain implementation at `3ab1c6e50c854857e92de0e8d65901c2b4990911`. It failed
on the assertion that an unstated assumption must not become a first-transition
claim. The corrected implementation passes the same test. Compilation failure
was not accepted as this regression's red proof.

The exhaustive synthetic queue test covers **each of the 20 possible first
failing transitions**. With compatible provider endpoint receipts, one decisive
attempt per state, and an explicit monotonic-target assumption, each diagnosis
uses at most **five native midpoint launches**. Provider evidence does not debit
that budget. Repeated-sample policies charge every additional attempt. This is
an algorithm and evidence-contract result, not a measured live-CI speedup.

Other tests reject or preserve stale context, foreign states, different failure
signatures, pass/fail conflicts, rewritten attempt IDs, pending launches,
insufficient equivalence, non-monotonic evidence and exhausted budgets. A plan
contains at most one proposed launch; it is not an atomic reservation.

## Installed executable

The actually installed binary passed `--help`, `schema input`, `schema report`
and `demo`. Replaying the demo returned:

```text
status: needs_evidence
next state: S10
recorded native launches: 0
exit code: 2
```

The input is explicitly synthetic and contains no real provider objects or
retained execution logs. The CLI did not run a predicate. Exit 2 is intentional:
missing evidence is not a successful localization or a green queue check.

## Remaining qualification and integration

Normal final-head PR gates and substantive review are separate from this receipt.
This run does not establish Windows/macOS behavior, declared-minimum-Rust
qualification, registry installation, authentication of supplied JSON, object
custody, isolated execution, launch reservation, live Check publication or the
controlled live 20-PR pilot.

The operated path remains #82 (capture/native packet conversion), #83 (phase
emitters/signatures/context), #58–#64/#80/#84 (execution/evidence/prefixes),
#91 (atomic admission and recovery), and #85–#86 (coordinator, Check and live
qualification). Do not close those obligations from this receipt.
