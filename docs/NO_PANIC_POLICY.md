# No-panic policy

This repository targets:

> No **unreceipted** panic-family behavior in production or tests.

"Panic family" means any of:

```
unwrap
expect
panic!
todo!
unimplemented!
unreachable!
unsafe string slicing
unchecked indexing/slicing
get(...).unwrap()
unwrap inside Result-returning functions
```

Test assertion macros (`assert!`, `assert_eq!`) are **not** in scope today.
A future migration to fallible test helpers may bring `panic_in_result_fn`
forward; for now `panic_in_result_fn` runs at `warn`.

## Two rails, one policy

Clippy raises panic-family findings at the source-shape level. The
semantic checker owns the **exception ledger**: location, family, selector,
owner, reason, classification, and expiry — none of which fit cleanly into
a `#[expect(...)]` attribute alone.

```
Clippy            -> immediate IDE / CI feedback
xtask check-no-panic-family -> blocking gate; reads policy/no-panic-allowlist.toml
```

Either rail can refuse to merge code. They reinforce each other; they do
not replace each other.

## Allowlist file: `policy/no-panic-allowlist.toml`

Schema (current `schema_version = "0.3"`):

```toml
schema_version = "0.3"

[[allow]]
id = "panic-0001"                      # stable id; never reuse
path = "crates/example/src/parser.rs"  # or `glob = "..."`
family = "unwrap"                       # one of the panic-family names
classification = "test_helper"          # see classifications below
owner = "parser"                        # who owns the burn-down
explanation = "Fixture setup; migrate to fallible builder."
expires = "2026-07-01"                  # required; expired entries fail check

[allow.selector]
kind = "method_call"                    # or "macro_call", "index_expr", etc.
container = "parses_boundary_fixture"   # function/test name
callee = "unwrap"                       # method or macro name
receiver_fingerprint = "std::fs::read_to_string(path)"  # advisory

[allow.last_seen]
line = 42                                # advisory only — not part of identity
column = 17
```

### Identity

```
identity = path (or glob) + family + selector
```

`last_seen.line` and `last_seen.column` are advisory hints for the human
reviewer. They drift as code changes. They are **never** the matching key.

### Classifications

| classification    | meaning                                                          |
| ----------------- | ---------------------------------------------------------------- |
| `test_helper`     | exception inside a test fn / fixture builder                     |
| `test_oracle`     | the assertion is the test contract                               |
| `fixture_setup`   | one-time fixture construction at test boot                       |
| `infallible`      | provably cannot panic given local invariants                     |
| `scaffold`        | placeholder for an unimplemented seam (`todo!`)                  |
| `baseline`        | initial baseline at policy adoption; must burn down              |

### Families

```
unwrap
expect
panic_macro
todo
unimplemented
unreachable
indexing
string_slice
get_unwrap
```

`family = "any"` is permitted only for the initial baseline entry.

## xtask commands

```
cargo xtask check-no-panic-family   # blocking; reads the allowlist
cargo xtask no-panic propose        # writes target/.../no-panic-proposed-allowlist.toml
cargo xtask policy-report           # roll-up across rails
```

Rules:

- `check-no-panic-family` fails if it finds an unallowlisted finding,
  an expired entry, or an entry whose target file no longer matches.
- `no-panic propose` never mutates `policy/no-panic-allowlist.toml`.
  Humans copy entries across by hand after review.

## Source-level suppressions

When a single call site needs an `#[expect(...)]`, reference the policy
id in the reason:

```rust
#[expect(
    clippy::unwrap_used,
    reason = "policy:no-panic:panic-0007 — see policy/no-panic-allowlist.toml"
)]
let parsed = candidate.parse::<u64>().unwrap();
```

This makes drift visible: a grep for `policy:no-panic:` lines up with
allowlist ids.
