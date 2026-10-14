# Build faultline from source

Use this path when the crates.io release is not yet available or you need the
current repository changes.

You need Git and Rust 1.93 or newer. See [rustup](https://rustup.rs/) if Rust is
not installed.

## Build and start

```bash
git clone https://github.com/EffortlessMetrics/faultline-rs.git
cd faultline-rs
cargo build --release -p faultline-cli
```

Run `target/release/faultline-cli --help` (`faultline-cli.exe` on Windows).
Then follow the [first-run guide](docs/book/src/tutorials/first-run.md) in the
repository you want to investigate.

## Contributor checks

```bash
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked
```

The repository's complete checks are documented in [TESTING.md](TESTING.md).
See [AGENTS.md](AGENTS.md) for contributor orientation and
[RELEASE.md](RELEASE.md) for publication checks.
