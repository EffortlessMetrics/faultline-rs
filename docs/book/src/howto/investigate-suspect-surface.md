# How to Read and Act on the Suspect Surface

This guide explains how to interpret the ranked suspect surface in a faultline report and use it to focus your investigation.

## Prerequisites

- A completed faultline run with output artifacts

## Step 1: Open the suspect surface

In the HTML report (`index.html`), scroll to the **Suspect Surface** section. In the JSON report (`analysis.json`), look for the `suspect_surface` array.

Each entry contains:

| Field | Description |
|-------|-------------|
| `path` | The changed file path |
| `priority_score` | Investigation priority (higher = investigate first) |
| `surface_kind` | Classification: source, tests, scripts, workflows, build-script, docs, lockfile, other |
| `change_status` | How the file changed: added, modified, deleted, renamed |
| `is_execution_surface` | Whether the file is a workflow, build script, or shell script |
| `owner_hint` | Suggested code owner (from CODEOWNERS or git-blame), or null |

## Step 2: Understand the scoring

Scores are deterministic and additive:

- Base: 100
- Execution surface: +200
- Deleted file: +150
- Renamed file: +100
- Source file: +50
- Test file: +25

A deleted workflow file scores 100 + 200 + 150 = 450. An ordinary modified source file scores 100 + 50 = 150.

## Step 3: Triage from the top

Work through the list from highest to lowest score:

1. **Execution surfaces** (workflows, build scripts, shell scripts) — these change how code is built or tested, and are common sources of unexpected breakage.
2. **Deleted and renamed files** — these can break imports, references, and build configurations.
3. **Source files** — the most common regression source, but lower priority than structural changes.
4. **Test files** — rarely the cause of a regression, but worth checking if the test itself was changed.

## Step 4: Use owner hints

The `owner_hint` field tells you who to contact. If a high-priority file is owned by another team, share the report with them directly.

## Step 5: Cross-reference with the outcome

The suspect surface shows all changed files between the boundary commits. Cross-reference with the specific first-bad commit (or suspect window) to narrow further:

```bash
git show --stat <first-bad-commit>
```

Files that appear in both the suspect surface and the first-bad commit's diff are your strongest leads.
