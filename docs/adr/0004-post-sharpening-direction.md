# ADR-0004: Post-sharpening product direction

## Status
Accepted.

## Context

The v0.1 product sharpening spec is complete. All five features — ranked suspect surface, Markdown dossier, flake-aware probing, reproduction capsules, and run comparison — are designed, specified, and implemented across 13 waves. The artifact contract (`analysis.json`, `index.html`, dossier) is stable and covered by golden tests and JSON Schema validation.

The tool is now well-designed but not yet battle-tested. Moving from "spec-complete" to "actively used" requires calibration against real regressions, integration into developer workflows, and hardening of the CI governance model.

## Decision

Post-sharpening work is organized into four tranches, roughly ordered by dependency:

1. **Operational calibration** — Dogfood faultline on real regressions across varied codebases. Tune the suspect ranking weights, flake detection thresholds, and confidence scoring against observed outcomes. Calibration findings feed back as threshold adjustments, not schema changes.

2. **Adoption surfaces** — Build the integration points that let developers use faultline without deep doctrine knowledge: a GitHub Action for CI-triggered bisection, PR comment workflows that surface results inline, shell completions for `faultline` subcommands, and a clean install path (`cargo install`, prebuilt binaries).

3. **Governance hardening** — Strengthen CI enforcement: contract-aware CI that validates artifact schema and golden snapshots on every PR, named gates that map to specific quality properties (e.g., `gate:schema-compat`, `gate:golden-match`), and mutation-on-diff that runs targeted mutation tests against changed code paths.

4. **Investigation loop** — Improve the operator's post-run experience: compact dossier mode for quick triage, signal assessment summaries that explain what the evidence supports, diff-runs Markdown that highlights what changed between two bisection runs, and `list-runs` for navigating run history.

Each tranche may proceed in parallel where dependencies allow, but calibration insights from tranche 1 inform the defaults shipped in tranches 2-4.

## Consequences

- Product focus shifts from feature design to calibration and polish. New feature work is paused until real-world feedback validates the current design.
- CI workflows become more granular and visible. Named gates replace monolithic pass/fail checks, making it easier to diagnose what broke and why.
- External adoption becomes possible without full doctrine buy-in. The GitHub Action and PR comment workflow provide a shallow on-ramp that doesn't require operators to understand the full architecture.
- The artifact contract is frozen. No schema-breaking changes to `analysis.json` or the dossier format without a version bump and migration path. This is a hard constraint for the duration of post-sharpening work.

## Related Patterns

- [Pattern: Artifact-First Boundary](../patterns/catalog.md#3-artifact-first-boundary) — the frozen artifact contract is a direct consequence of this pattern; schema stability is now an operational constraint, not just a design preference
- [Pattern: Truth Core / Translation Edge](../patterns/catalog.md#1-truth-core--translation-edge) — calibration adjustments (thresholds, weights) stay in domain crates; adoption surfaces are new translation edges that wrap the same truth core
