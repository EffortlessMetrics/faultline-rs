# Retained probe attempts

The history application journals each completed raw execution before cleanup,
the next retry, or aggregation. `attempts/<20-digit sequence_index>.json` uses
create-new writes and file synchronization. Identical writes are idempotent;
conflicting identities, non-regular entries and malformed files fail closed.
`observations.json` remains only a derived per-commit compatibility cache.

`probe_attempts` contains original metadata. Separate `probe_aggregates` link to
run-local attempt IDs and the `unanimous_complete_v1` rule. Multi-attempt search
projections borrow no exit code, output, working directory or duration. Mixed
outcomes are indeterminate even when the legacy stability threshold would admit
a majority. Unstable endpoints produce inconclusive reports, not exact blame.

New reports emit schema 0.4.0. Older JSON remains readable with empty new fields;
this does not invent originals for old aggregate records. Cached decisions without
matching original journal evidence are rejected. `--force` recollects while
retaining old attempts, and new IDs remain monotonic. A normal resume verifies
cached decisions against their retained originals. Interrupted batches retain
completed attempts but are recollected; they are not silently marked complete.
Explicit `--fresh` retains its existing destructive meaning and deletes the run.

The existing single-writer operating boundary still applies. This change is not
atomic launch admission, distributed locking, power-loss recovery, or complete
untruncated logging. A partial journal write blocks replay rather than being
silently skipped. Process errors with no returned observation and unfinished
workers require #59/#91 lifecycle records. Actual launch-budget enforcement is
still #64; the old revision-count cap is not an attempt budget. Reuse identities
remain #63 and filesystem/worktree ownership #61.

JSON, Markdown, HTML, SARIF and JUnit preserve the evidence/aggregate distinction.
The existing redaction path also scrubs the newly exported raw attempt output.
No registry release, live queue worker or required-check cutover is introduced.

The legacy stability-threshold option remains accepted for request compatibility,
but no threshold turns mixed executions into a decisive boundary. CLI help states
that rule. New evidence-bearing HTML/Markdown reports do not advertise inferred
full-log links merely because captured output ends in a truncation marker. The
captured output is retained in the JSON projection; complete log custody is #60.

The CLI now passes `force`, `fresh` and `no_render` into `LocalizeOptions`;
previously those parsed run-mode flags never reached the application. A real
Git/CLI regression exercises three attempts per state, no-execution resume,
force retaining previous attempts, and explicit fresh creating a new journal.
This repairs flag wiring, not the broader filesystem ownership work in #61.
