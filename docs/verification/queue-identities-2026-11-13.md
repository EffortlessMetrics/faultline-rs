# Queue identifier qualification — 2026-11-13

Subject: `d168c05992eca029df5644a38247f1428e2744db`, following
`d69900ae803af414b7f2db94efca8bf8cb3cfc63` in PR #92.

The application previously accepted sequence identities that its own receipt
validator rejected. It could therefore propose a state for which no matching
valid receipt was possible. Diagnosis now applies that same bounded identifier
validation to the snapshot and every state ID, commit and tree before planning.
It rejects incompatible identities rather than silently rewriting them.

## Executed evidence

[Run 43405894866](https://github.com/EffortlessMetrics/faultline-rs/actions/runs/43405894866)
executed the negative regression against the exact previous implementation:
`planned_state_identifiers_must_be_receiptable` compiled and failed. The bounded
Unicode-identity control passed. With the correction, both tests and the complete
integration/queue/CLI suite passed: **74 tests, zero failures**.

The negative test covers newline, NUL and over-limit identities across snapshot,
state ID, commit and tree. Existing 20-position localization and property tests
were retained. Strict Clippy for `faultline-queue`, formatting and diff checks
also passed. The same queue source was included in the subsequent producer
qualification's Windows stable and Linux Rust 1.93.0 native test runs.

The `queue-identity-proof` artifact is ID `14026404842`, ZIP SHA-256
`ec300c7a8adb95e3a01ea3572dba03323138a437027b7b05417555e8c7c2dcfc`.
Its downloaded bytes were hash-checked; the patch, qualified source identifier
and red/green logs were inspected. The artifact includes the source Git bundle.

The workflow recipe head is not the code subject: the queue job explicitly
checks out the input SHA, adds the tests and correction, verifies them and
packages the qualified commit. The later publisher checks its parent and the
candidate branch head before advancing that branch. Normal final-head PR gates
remain separate; this document does not declare them passed in advance.

## Scope

This changes identifier consistency, not provider authentication, object custody,
probe execution or queue control. The input/report schema remains diagnosis
version 0.1.0. The separate history-producer report migration to 0.4.0 does not
change that queue contract. No release, merge or live deployment is implied.

Earlier qualification remains attached to its original source in
[queue-review-2026-11-12.md](queue-review-2026-11-12.md).
