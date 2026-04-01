# How Suspect Surface Ranking Works

This page explains the algorithm behind faultline's ranked suspect surface — how changed files are scored, sorted, and annotated with owner hints.

## The problem

When faultline identifies a regression window, the diff between the boundary commits often contains many changed files. A flat alphabetical list forces the operator to scan every file to decide where to start investigating. The suspect surface solves this by ranking files by investigation priority.

## Scoring algorithm

Each changed file receives a priority score computed from additive bonuses on a base score:

| Factor | Score |
|--------|-------|
| Base (every file) | 100 |
| Execution surface (workflow, build.rs, shell script) | +200 |
| Deleted file | +150 |
| Renamed file | +100 |
| Source file (.rs, .py, .js, etc.) | +50 |
| Test file | +25 |

Bonuses stack. A deleted build script scores 100 (base) + 200 (execution surface) + 150 (deleted) = 450.

### Why this ordering?

The scoring reflects empirical investigation priorities:

- **Execution surfaces** change how code is built, tested, or deployed. A modified CI workflow or deleted build script can break everything downstream without touching application logic.
- **Deletions** remove code that other files may depend on. Missing files cause build failures, broken imports, and runtime errors.
- **Renames** can break import paths, configuration references, and build system assumptions, especially in languages without automatic refactoring.
- **Source files** are the most common regression source but are lower priority than structural changes because their impact is usually more localized.
- **Test files** are rarely the cause of a production regression, but changes to test infrastructure can affect CI outcomes.

## Determinism

The ranking algorithm is deterministic: identical inputs always produce identical output. Ties are broken by ascending lexicographic path order. This means results are reproducible across runs and machines.

## Surface kind classification

Each file is classified using the existing `surface_kind()` function from `faultline-surface`:

| Kind | Examples |
|------|----------|
| `source` | `.rs`, `.py`, `.js`, `.go`, `.java` |
| `tests` | Files in `tests/`, `test_*.py`, `*_test.go` |
| `scripts` | `.sh`, `.bash`, `.ps1` |
| `workflows` | `.github/workflows/*.yml` |
| `build-script` | `build.rs`, `Makefile`, `CMakeLists.txt` |
| `docs` | `.md`, `.rst`, `.txt` in doc directories |
| `lockfile` | `Cargo.lock`, `package-lock.json`, `yarn.lock` |
| `other` | Everything else |

The `is_execution_surface` flag is `true` for workflows, scripts, and build scripts.

## Owner hints

Each suspect entry includes an optional `owner_hint` — a suggested person or team to contact about the file. Owner hints are derived from two sources, in priority order:

1. **CODEOWNERS file** — If a `.github/CODEOWNERS` or `CODEOWNERS` file exists in the repository root, faultline parses it and matches each changed path against the patterns. The first matching owner is used.

2. **Git-blame frequency** — If no CODEOWNERS file exists, faultline falls back to git-blame heuristics: for each changed file, it finds the most-frequent committer in the last 90 days.

If neither source produces an owner, the `owner_hint` is `null`.

## Where the data lives

The suspect surface is stored in the `AnalysisReport` as a `suspect_surface` array. Each entry is a `SuspectEntry` with fields: `path`, `priority_score`, `surface_kind`, `change_status`, `is_execution_surface`, and `owner_hint`.

The HTML report renders the suspect surface as a prioritized list with visual distinction for execution surfaces. The Markdown dossier includes the top 10 entries. The JSON artifact includes the full array.

## Relationship to SurfaceSummary

The existing `SurfaceSummary` provides coarse bucketing (counts of source, test, doc, etc. files). The suspect surface is a complementary, finer-grained view that ranks individual files by investigation priority. Both are included in the report.
