# Building faultline

## Prerequisites

- **Rust stable** (edition 2024) — install via [rustup](https://rustup.rs/)
- **git** on PATH — faultline shells out to the system `git` binary

## Build

```bash
cargo build --workspace
```

## Test

```bash
cargo test --workspace
```

Run a specific crate's tests:

```bash
cargo test -p faultline-localization
```

## Formatting

```bash
cargo fmt --all --check   # verify
cargo fmt --all           # fix
```

## Linting

```bash
cargo clippy --workspace -- -D warnings
```

## Run the CLI

```bash
cargo run -p faultline-cli -- --help
```

## Notes

- faultline shells out to the system `git` binary for history linearization and worktree management.
- The probe adapter runs your predicate command in disposable linked worktrees under `.faultline/scratch/`.
- Reports are emitted as `analysis.json` (structured) and `index.html` (human-readable) in the output directory.
