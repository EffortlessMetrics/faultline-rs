# Building

This workspace targets stable Rust and uses common crates only (`clap`, `serde`, `serde_json`, `thiserror`).

## Local build

```bash
cargo test
cargo run -p faultline-cli -- --help
```

## Notes

- `faultline` shells out to the system `git` binary.
- The probe adapter runs your command in disposable linked worktrees under `.faultline/scratch/`.
- Reports are emitted as `analysis.json` and `index.html`.
