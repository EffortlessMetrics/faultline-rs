# Non-Rust file policy

Rust + xtask is the default implementation path for this repository.
Non-Rust files require an explicit receipt in `policy/non-rust-allowlist.toml`.

## What "non-Rust" means here

Tracked files whose path does not match:

```
*.rs
*.md
Cargo.toml / Cargo.lock          (these are policy-allowed by default entry)
```

Markdown documentation is exempt. Everything else — YAML, TOML config,
JSON schemas, HTML, snapshots, license texts, ignore files — must appear
in the allowlist.

## Allowlist schema

```toml
schema_version = "1.0"

[[allow]]
glob = ".github/workflows/*.yml"
kind = "ci_declarative"
owner = "release/ci"
surface = "ci"
classification = "production"
reason = "GitHub Actions workflow definitions are platform-required YAML."
covered_by = ["cargo xtask check-workflows"]
# Optional:
# expires = "2026-09-01"
# generated_by = "cargo xtask generate-schema"
# retired = false
```

### Required fields

- `glob` *or* `path` — what the entry matches.
- `kind` — short symbolic surface kind (e.g. `ci_declarative`, `generated_metadata`,
  `test_fixture`, `build_metadata`, `legal`, `policy`).
- `owner` — team or area responsible for the surface.
- `surface` — high-level surface name (`ci`, `build`, `docs`, `schema`,
  `snapshots`, `policy`, ...).
- `classification` — one of `production`, `test`, `tooling`, `generated`,
  `config`.
- `reason` — why the file exists in this repo despite the Rust-default rule.
- `covered_by` — list of commands that exercise the file. May be empty for
  pure repo-policy artifacts (LICENSE, .gitignore).

### Optional fields

- `expires` — ISO date. Once past, `check-file-policy` fails.
- `generated_by` — command that regenerates the file (informational; pairs
  well with `classification = "generated"`).
- `retired` — `true` allows the entry to remain even if it currently
  matches no tracked file (e.g. while a removal lands in stages).

## xtask command

```
cargo xtask check-file-policy
```

Fails on:

1. any tracked non-Rust, non-Markdown file with no matching allow entry,
2. any allow entry whose `expires` has passed,
3. any allow entry that matches no file unless `retired = true`.

Writes `target/policy/file-policy.{md,json}`.

## Adding a new non-Rust surface

1. Add the file(s).
2. Add a corresponding `[[allow]]` entry to `policy/non-rust-allowlist.toml`.
3. If the surface is exercised by a check or script, list it in `covered_by`.
4. If the surface is generated, set `classification = "generated"` and
   include `generated_by`.
5. Run `cargo xtask check-file-policy` and commit.

## Why this exists

Without a non-Rust gate, repos drift toward shell scripts, ad-hoc Python,
and config sprawl that nobody owns. The allowlist makes surfaces explicit,
gives them owners, and forces a reason. Removing a surface becomes a real
review event instead of a silent prune.
