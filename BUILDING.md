# Build faultline-rs from source

Use this path when the crates.io release is not yet available or you need the
current repository changes.

You need Git and Rust 1.93 or newer. See [rustup](https://rustup.rs/) if Rust is
not installed.

## Build and start

```bash
git clone https://github.com/EffortlessMetrics/faultline-rs.git
cd faultline-rs
cargo build --release -p faultline-rs
```

Run `target/release/faultline-rs --help` (`faultline-rs.exe` on Windows).
Then follow the [first-run guide](docs/book/src/tutorials/first-run.md) in the
repository you want to investigate.

## Codex Cloud setup

In **Settings > Codex Cloud > Environments > Edit**, select
`EffortlessMetrics/faultline-rs` and paste this Bash block into **Install
script**. Run it from the repository root. The legacy UI calls this field
**Setup script**. [Cloud environments](https://learn.chatgpt.com/docs/environments/cloud-environments)

The block uses the Linux image's Git, C linker and standard command-line
tools. It provisions Rust stable with rustfmt and Clippy, the exact Rust
1.93.1 toolchain used by late qualification, the latest uv, and Node.js from
the official `latest` channel for the newest final release. That release
may already have entered LTS.
[Late qualification](.github/workflows/late-qualification.yml) · [Node.js releases](https://nodejs.org/en/about/previous-releases) · [uv installer options](https://docs.astral.sh/uv/reference/installer/)

```bash
set -euo pipefail
download() { curl --proto "=https" --tlsv1.2 -fsSL "$@"; }

export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
if ! command -v rustup >/dev/null 2>&1; then
  download https://sh.rustup.rs | sh -s -- -y --default-toolchain none --no-modify-path
fi
rustup toolchain install stable --profile minimal --component rustfmt --component clippy
rustup toolchain install 1.93.1 --profile minimal
rustup run 1.93.1 cargo fetch --locked

tools_dir="$HOME/.local/opt/faultline-cloud"
mkdir -p "$tools_dir"
case "$(uname -m)" in
  x86_64) node_arch=x64 ;;
  aarch64|arm64) node_arch=arm64 ;;
  *) printf 'Unsupported Linux architecture for Node.js\n' >&2; exit 1 ;;
esac

(
  node_dist=https://nodejs.org/dist
  scratch=$(mktemp -d "$tools_dir/node-download.XXXXXX")
  trap 'rm -rf -- "$scratch"' EXIT
  cd "$scratch"
  download "$node_dist/latest/SHASUMS256.txt" |
    awk -v arch="$node_arch" '$2 ~ ("^node-v[0-9]+\\.[0-9]+\\.[0-9]+-linux-" arch "\\.tar\\.xz$")' > node.sha256
  read -r checksum archive < node.sha256
  node_dir="${archive%.tar.xz}"
  node_version="${node_dir#node-}"
  if [[ ! -x "$tools_dir/$node_dir/bin/node" ]]; then
    download "$node_dist/${node_version%-linux-*}/$archive" -o "$archive"
    sha256sum --check node.sha256
    tar -xJf "$archive"
    mv -T -- "$node_dir" "$tools_dir/$node_dir"
  fi
  ln -sfnT -- "$node_dir" "$tools_dir/node-current"
)

download https://astral.sh/uv/install.sh | UV_UNMANAGED_INSTALL="$tools_dir/uv" sh
```

Rustup manages the toolchains. Use `cargo +stable fmt` for general tooling.
The workspace MSRV is Rust 1.93. Use the 1.93.1 toolchain for the locked checks
below. Setup preserves existing Rust defaults, directory overrides
and shell profiles. The repository pins the workflow linter to
`jactionlint==2.5.0` for its separate workflow checks.
[Workspace manifest](Cargo.toml) · [Pinned workflow checks](AGENTS.md#command-surface)

Installation needs HTTPS access to the official Rust, Node.js and Astral
release endpoints and the Cargo registry.

### Readiness check

From the repository root in a fresh task, run this block with agent internet
access off. The setup script's PATH export applies only to its own shell.
This block selects the prepared tools for the current shell and checks the
dependency cache against `Cargo.lock` without compiling. It checks the
prepared Node.js and uv binaries by path so tools on the image cannot
mask a missing installation.
It also disables Rustup's automatic toolchain installation in this shell
and checks that both toolchains are already installed.

```bash
tools_dir="$HOME/.local/opt/faultline-cloud"
export PATH="$tools_dir/node-current/bin:$tools_dir/uv:${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
export RUSTUP_AUTO_INSTALL=0
git --version &&
  cc --version &&
  rustup which --toolchain stable rustc >/dev/null &&
  rustup which --toolchain 1.93.1 rustc >/dev/null &&
  rustup which --toolchain 1.93.1 cargo >/dev/null &&
  rustup run stable rustc --version &&
  rustup run 1.93.1 rustc --version &&
  "$tools_dir/node-current/bin/node" --version &&
  "$tools_dir/uv/uv" --version &&
  rustup run 1.93.1 cargo metadata --offline --locked --format-version 1 >/dev/null
```

If a prepared binary, toolchain, or cached crate is missing, rerun the Install
script during environment setup. If Cargo says the lockfile needs an update,
reconcile the checkout's manifests and `Cargo.lock` so they agree, then run
readiness again.

### Optional CLI smoke test

In the same shell, build and run the CLI offline. Cargo writes normal build
artifacts and keeps `Cargo.lock` fixed.

```bash
RUSTUP_AUTO_INSTALL=0 rustup run 1.93.1 cargo run --offline --locked -p faultline-rs -- --version
```

## Contributor checks

```bash
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked
```

The repository's complete checks are documented in [TESTING.md](TESTING.md).
See [AGENTS.md](AGENTS.md) for contributor orientation and
[RELEASE.md](RELEASE.md) for publication checks.
