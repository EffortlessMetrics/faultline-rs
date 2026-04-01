# faultline-rs Justfile
# Run `just` or `just help` to see all available recipes.

# --------------------------------------------------------------------------- #
# Default / Help
# --------------------------------------------------------------------------- #

# Show all available recipes
default: help

# List every recipe with its description
help:
    @just --list

# --------------------------------------------------------------------------- #
# Common Development Recipes
# --------------------------------------------------------------------------- #

# Run the faultline CLI with arbitrary arguments
run *ARGS:
    cargo run -p faultline-cli -- {{ARGS}}

# Build the entire workspace
build:
    cargo build --workspace

# Run workspace tests (extra args forwarded to cargo test)
test *ARGS:
    cargo test --workspace {{ARGS}}

# Run tests for a single crate (extra args forwarded)
test-crate CRATE *ARGS:
    cargo test -p {{CRATE}} {{ARGS}}

# Run clippy with deny-warnings across the workspace
clippy:
    cargo clippy --workspace -- -D warnings

# Format all code in the workspace
fmt:
    cargo fmt --all

# Check formatting without modifying files
fmt-check:
    cargo fmt --all --check

# Build rustdoc for the workspace (no dependencies)
doc:
    cargo doc --workspace --no-deps

# --------------------------------------------------------------------------- #
# CLI Convenience Recipes
# --------------------------------------------------------------------------- #

# List analysis runs stored in REPO (defaults to current directory)
list-runs REPO=".":
    cargo run -p faultline-cli -- list-runs --repo {{REPO}}

# Clean analysis runs stored in REPO (defaults to current directory)
clean-runs REPO=".":
    cargo run -p faultline-cli -- clean --repo {{REPO}}

# --------------------------------------------------------------------------- #
# CI / Quality Gate Recipes (via xtask)
# --------------------------------------------------------------------------- #

# Alias for ci-fast
ci: ci-fast

# Fast CI tier: fmt + clippy + test
ci-fast:
    cargo xtask ci-fast

# Full CI tier: ci-fast + golden + schema-check
ci-full:
    cargo xtask ci-full

# Build CLI and run against fixture repo
smoke:
    cargo xtask smoke

# Run and update golden/snapshot tests
golden:
    cargo xtask golden

# Run cargo-mutants on configured surfaces
mutants:
    cargo xtask mutants

# Run fuzz targets (default 60s)
fuzz duration="60":
    cargo xtask fuzz --duration {{duration}}

# Build docs and check links
docs:
    cargo xtask docs-check

# Run cargo-deny + cargo-audit + cargo-semver-checks
release-check:
    cargo xtask release-check

# --------------------------------------------------------------------------- #
# Scaffolding & Schema (via xtask)
# --------------------------------------------------------------------------- #

# Generate boilerplate for new repo artifacts
scaffold *args:
    cargo xtask scaffold {{args}}

# Regenerate the analysis-report JSON schema from Rust types
generate-schema:
    cargo xtask generate-schema

# Verify scenario atlas entries match workspace tests
check-scenarios:
    cargo xtask check-scenarios

# --------------------------------------------------------------------------- #
# Export Recipes (via xtask)
# --------------------------------------------------------------------------- #

# Export a Markdown dossier from a run directory
export-markdown *ARGS:
    cargo xtask export-markdown {{ARGS}}

# Export SARIF from a run directory
export-sarif *ARGS:
    cargo xtask export-sarif {{ARGS}}

# Export JUnit XML from a run directory
export-junit *ARGS:
    cargo xtask export-junit {{ARGS}}
