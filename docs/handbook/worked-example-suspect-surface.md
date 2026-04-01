# Worked Example: Investigating a Regression with the Ranked Suspect Surface

This walkthrough demonstrates how to use faultline's ranked suspect surface to investigate a regression efficiently. Instead of scanning a flat list of changed files, you follow the priority-ranked list to focus on the most likely culprits first.

## Scenario

Your CI pipeline reports that `cargo test --lib` fails on `main` but passed on the `v0.9.0` tag. You suspect a recent merge introduced the regression, but the merge touched 47 files across 12 commits.

## Step 1: Run faultline

```bash
faultline-cli \
  --repo . \
  --good v0.9.0 \
  --bad main \
  --cmd "cargo test --lib" \
  --output-dir regression-report
```

Faultline narrows the window and produces `analysis.json`, `index.html`, and (with `--markdown`) a Markdown dossier.

## Step 2: Open the report and find the suspect surface

Open `regression-report/index.html` in a browser. Scroll to the **Suspect Surface** section. You'll see a ranked table like:

| Priority | Path | Kind | Status | Exec Surface | Owner |
|----------|------|------|--------|--------------|-------|
| 350 | `.github/workflows/ci.yml` | workflows | modified | ✓ | @infra-team |
| 300 | `build.rs` | build-script | deleted | ✓ | @core-team |
| 250 | `src/config.rs` | source | renamed | — | @alice |
| 150 | `src/lib.rs` | source | modified | — | @bob |
| 125 | `tests/integration.rs` | tests | modified | — | @alice |

The scoring rules are deterministic:
- Base score: 100
- Execution surface (workflow, build.rs, shell script): +200
- Deleted file: +150
- Renamed file: +100
- Source file: +50
- Test file: +25

## Step 3: Triage from the top

Start with the highest-priority entries:

1. **`.github/workflows/ci.yml` (350)** — A modified workflow file is an execution surface. Check if the CI configuration change affected how tests are run (different environment, missing dependency, changed test command).

2. **`build.rs` (300)** — A deleted build script is high priority. If `build.rs` generated code or set environment variables that tests depend on, its removal could be the root cause.

3. **`src/config.rs` (250)** — A renamed source file. Check if the rename broke import paths or if the rename was accompanied by logic changes.

## Step 4: Use owner hints for escalation

The `Owner` column tells you who to ask. If you determine that `build.rs` deletion is the likely cause and `@core-team` owns it, you can immediately ping them with the relevant context.

If no CODEOWNERS file exists, faultline falls back to git-blame frequency (most-frequent committer in the last 90 days).

## Step 5: Cross-reference with the observation timeline

The report also shows which commit was identified as the first-bad (or suspect window). Cross-reference the suspect surface paths with the diff of that specific commit:

```bash
git diff <last-good>..<first-bad> -- build.rs
```

This confirms whether the high-priority file was actually changed in the regression-introducing commit.

## Step 6: Share findings

Use the Markdown dossier (`--markdown` flag) to paste the suspect surface into a PR comment or incident thread:

```bash
faultline-cli \
  --repo . --good v0.9.0 --bad main \
  --cmd "cargo test --lib" \
  --output-dir regression-report \
  --markdown
```

The Markdown output includes the ranked suspect surface, observation timeline, and reproduction command — everything a reviewer needs without opening HTML or parsing JSON.

## Key takeaways

- The suspect surface saves time by ranking files by investigation priority instead of presenting a flat alphabetical list.
- Execution surfaces, deletions, and renames are ranked highest because they are most likely to cause unexpected breakage.
- Owner hints let you route investigation to the right person immediately.
- The ranking is deterministic — identical inputs always produce identical rankings, so results are reproducible.
