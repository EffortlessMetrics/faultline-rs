# faultline Architecture Handbook

This handbook is the entry point for understanding the faultline repository's architecture, patterns, testing strategy, and operational practices.

## Maintainer Playbooks

| Playbook | Purpose |
|----------|---------|
| [Reviewing Failing Property Tests](reviewing-failing-property-tests.md) | Investigate a counterexample, triage root cause, fix or escalate |
| [Deciding Test Technique](deciding-test-technique.md) | Decision tree for property vs unit vs golden vs fuzz vs mutation |
| [Bumping `schema_version`](bumping-schema-version.md) | When and how to bump `AnalysisReport` schema version |
| [Handling Breaking Changes](handling-breaking-changes.md) | Semver, migration, backward compat, CI checks, release process |

## Worked Examples

| Example | Purpose |
|---------|---------|
| [Investigating a Regression with the Suspect Surface](worked-example-suspect-surface.md) | Walkthrough of using the ranked suspect surface to triage a regression |
| [Handling a Flaky Test with Flake-Aware Probing](worked-example-flake-detection.md) | Walkthrough of using retries and stability thresholds for flaky predicates |

## Quick Links

| Document | Purpose |
|----------|---------|
| [Pattern Catalog](../patterns/catalog.md) | 10 named patterns governing architecture, testing, and operations |
| [ADR Index](../adr/) | Architecture Decision Records (ADR-0001 through ADR-0003+) |
| [ADR Template](../adr/TEMPLATE.md) | Template for new ADRs with Related Patterns section |
| [Scenario Atlas](../scenarios/scenario_index.md) | Flat index of every test in the workspace |
| [Behavior Map](../scenarios/behavior_map.md) | Five-way cross-reference: requirement → ADR → scenario → fixture → artifact |
| [Crate Map](../crate-map.md) | Every workspace crate with tier, dependencies, and responsibility |
| [Verification Matrix](../verification-matrix.md) | Per-crate verification techniques and testing budgets |
| [Architecture](../architecture.md) | High-level hexagonal architecture and crate boundaries |
| [Mission and Vision](../mission-and-vision.md) | Project mission, vision, and design principles |
| [Predicate Contract](../predicate-contract.md) | Exit code classification and predicate guidance |
| [Non-Goals](../non-goals.md) | What faultline is not |

## Architecture at a Glance

faultline follows a hexagonal (ports-and-adapters) architecture with two pure domains, four port traits, five infrastructure adapters, one application orchestrator, and one CLI entry point. See the [Crate Map](../crate-map.md) for the full dependency graph.

The key architectural invariant is **Truth Core / Translation Edge**: domain logic lives in pure crates with no I/O, and all infrastructure concerns are pushed to adapter crates behind port traits. This is documented in [ADR-0001](../adr/0001-hexagonal-architecture.md) and the [Pattern Catalog](../patterns/catalog.md).

## Testing Strategy

The [Verification Matrix](../verification-matrix.md) assigns verification techniques by crate tier:

- **Domain crates** → property tests (100 cases per property) + unit tests
- **Adapter crates** → BDD scenarios + property tests + golden tests
- **App crate** → integration scenarios + mutation testing
- **CLI crate** → smoke tests + golden tests + property tests

The [Scenario Atlas](../scenarios/scenario_index.md) catalogs every test with its problem statement, fixture, invariant, and cross-references.

## Contributing

When making changes, consult the [Pattern Catalog](../patterns/catalog.md) to ensure your change follows established patterns. Key patterns to keep in mind:

- **Proof-Carrying Change** — every domain change needs an accompanying test
- **Artifact-First Boundary** — artifact structure changes require schema regeneration and golden review
- **Scenario Atlas** — new tests must be added to the scenario index
- **Human Review Gate** — property test failures require human review, not automated suppression

## Related Documents

- [AGENTS.md](../../AGENTS.md) — Agent-facing onboarding guide and primary entry point
- [TESTING.md](../../TESTING.md) — Testing procedures, CI tiers, and how-to guides
- [RELEASE.md](../../RELEASE.md) — Release process, version bumps, and supply-chain checks
- [MAINTAINERS.md](../../MAINTAINERS.md) — Code ownership, review expectations, and escalation paths
