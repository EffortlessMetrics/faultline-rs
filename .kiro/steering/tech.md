# Tech Stack

## Language & Toolchain

- Rust (stable)
- Cargo workspace with resolver v2
- Edition 2024

## Core Dependencies

- `serde` / `serde_json` - serialization
- `clap` (v4.5, derive feature) - CLI argument parsing
- `thiserror` - error type derivation

## External Runtime Dependencies

- System `git` binary (shelled out, not libgit2)
- Disposable linked worktrees under `.faultline/scratch/`

## Build Commands

```bash
# Run all tests
cargo test

# Build entire workspace
cargo build

# Run CLI with help
cargo run -p faultline-cli -- --help

# Run specific crate tests
cargo test -p faultline-localization
```

## Output Artifacts

- `analysis.json` - structured report
- `index.html` - human-readable report

## Predicate Exit Codes

- `0` → pass
- `125` → skip/untestable revision
- non-zero → fail
- timeout → indeterminate
