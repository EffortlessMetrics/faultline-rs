# faultline-ports

Trait boundaries for infrastructure adapters.

Primary files:
- `src/lib.rs`

Responsibilities:
- Preserve trait contracts and argument/return semantics for adapters.
- Keep trait methods narrowly scoped to keep domain decoupled from I/O.

Validation:
- `cargo test -p faultline-ports`

