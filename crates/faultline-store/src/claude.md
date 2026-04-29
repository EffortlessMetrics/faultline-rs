# faultline-store Source

Persistent run store implementation module.

Primary file:
- `crates/faultline-store/src/lib.rs`

Keep atomic write patterns and lock cleanup consistent to avoid partial-run corruption.

Validation:
- `cargo test -p faultline-store`

