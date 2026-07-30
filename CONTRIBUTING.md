# CONTRIBUTING.md — How to Contribute to faultline

## Quick Start

**Prerequisites:** Rust stable (edition 2024), Git.

```bash
git clone https://github.com/<org>/faultline-rs.git
cd faultline-rs
cargo build --workspace
cargo test --workspace
```

Verify everything passes end-to-end:

```bash
cargo xtask ci-fast   # fmt + clippy + test
```

Tool versions are pinned in [.mise.toml](.mise.toml). If you use [mise](https://mise.jdx.dev/), run `mise install` to get the right toolchain.

## Finding Work

- Check [GitHub Issues](../../issues) for open tasks. Issues labeled `good first issue` are scoped for newcomers.
- **Adapter crates** (`faultline-git`, `faultline-store`, `faultline-render`, `faultline-sarif`, `faultline-junit`) are the best starting point — they have clear boundaries and self-contained tests.
- The [crate map](docs/crate-map.md) shows every crate's responsibility and tier.

## Development Workflow

1. **Branch from `main`** using the naming convention:
   - `feat/<description>` — new features
   - `fix/<description>` — bug fixes
   - `docs/<description>` — documentation changes

2. **Run `cargo xtask ci-fast` before pushing.** This is the same check that runs on every push in CI.

3. **Open a PR** against `main`. PRs should:
   - Have a clear description of what changed and why.
   - Include tests for new behavior.
   - Pass `ci-fast` (CI runs `ci-full` on PRs, which adds golden snapshot and schema checks).

## Code Style

- Follow existing patterns in the crate you are modifying. When in doubt, look at neighboring files.
- No unnecessary abstractions. If a function is only called once and the indirection does not clarify intent, inline it.
- **Domain crates stay pure** — no I/O, no filesystem, no network. All infrastructure goes through port traits in adapter crates. See [docs/architecture.md](docs/architecture.md).
- Run `cargo fmt` before committing. CI enforces `cargo fmt --check`.
- Zero clippy warnings: `cargo clippy --workspace -- -D warnings`.

## Testing Requirements

Every change that adds or modifies behavior needs tests. The specific requirements depend on what you are changing:

- **Property tests** must use at least 100 cases (`ProptestConfig { cases: 100, .. }`).
- **Golden/snapshot tests** use [insta](https://insta.rs/). When your change alters an artifact, run `cargo insta review` to accept the new snapshot and commit the updated `.snap` file.
- **Every new test must have a scenario atlas entry** in [docs/scenarios/scenario_index.md](docs/scenarios/scenario_index.md). CI enforces this.

See [TESTING.md](TESTING.md) for the full verification guide, including how to add property tests, fixture scenarios, and golden artifacts.

## PR Checklist

Before requesting review, confirm:

- [ ] `cargo xtask ci-fast` passes (formatting, clippy, tests)
- [ ] New tests have entries in the [scenario atlas](docs/scenarios/scenario_index.md)
- [ ] Golden snapshots are updated if artifact output changed (`cargo insta review`)
- [ ] No clippy warnings
- [ ] Domain crates remain free of I/O dependencies

## Architecture Quick Reference

faultline uses a **hexagonal (ports-and-adapters) architecture** across 15 crates:

- **Domain crates** (`faultline-codes`, `faultline-types`, `faultline-localization`, `faultline-surface`) contain pure logic with no I/O.
- **Port traits** (`faultline-ports`) define outbound interfaces.
- **Adapter crates** (`faultline-git`, `faultline-probe-exec`, `faultline-store`, `faultline-render`, `faultline-sarif`, `faultline-junit`) implement ports with real infrastructure.
- **Application** (`faultline-app`) orchestrates domain logic through ports.
- **Entry** (`faultline-cli`) wires adapters to ports and runs the CLI.

Dependencies always point inward: adapters depend on ports and types, never the reverse.

For the full picture, see [docs/architecture.md](docs/architecture.md) and [AGENTS.md](AGENTS.md).

## Communication

- Use [GitHub Issues](../../issues) for bug reports, feature requests, and questions.
- If you are unsure whether a change is welcome, open an issue first to discuss the approach before writing code.

## Further Reading

| Document | Purpose |
|----------|---------|
| [AGENTS.md](AGENTS.md) | Full contributor onboarding, command surface, escalation rules |
| [TESTING.md](TESTING.md) | Verification matrix, CI tiers, testing how-tos |
| [MAINTAINERS.md](MAINTAINERS.md) | Code ownership, review expectations, escalation path |
| [RELEASE.md](RELEASE.md) | Release process, versioning policy |
| [docs/architecture.md](docs/architecture.md) | Hexagonal architecture and crate boundaries |
| [docs/crate-map.md](docs/crate-map.md) | Every crate with tier, dependencies, responsibility |

## License

This project is licensed under [Apache-2.0](LICENSE). By contributing, you agree that your contributions will be licensed under the same terms.
