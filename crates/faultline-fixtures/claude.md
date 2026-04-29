# faultline-fixtures

Shared test fixture builders for both unit and adapter-level tests.

Primary files:
- `src/lib.rs`
- `src/arb.rs`

Responsibilities:
- Keep generated fixture behavior deterministic and minimal unless test goals demand extra complexity.
- Treat Git fixture helpers and property test generators as shared assets; avoid semantic changes without updating call sites.

Validation:
- `cargo test -p faultline-fixtures`
- Use fixture builders to construct temporary repos only inside tests unless explicitly required.

