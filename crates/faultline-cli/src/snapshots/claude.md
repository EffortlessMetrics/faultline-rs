# CLI Snapshots

Snapshot artifacts for clap help and other golden outputs.

Primary files:
- `faultline_rs__tests__cli_help.snap`

Responsibilities:
- Update snapshots only alongside intentional CLI help/message changes.
- Do not manually edit `.snap` payloads unless you also update formatter assumptions.

Validation:
- `cargo insta test -p faultline-rs`
- `cargo insta review` to accept intentional snapshot updates

