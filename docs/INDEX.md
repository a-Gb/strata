# Documentation

The numbered documents describe product and architecture in reading order.
`20-implementation-status.md` is the maintained source of truth for which parts
of that architecture run today.

## Product and system

1. [`00-product-brief.md`](00-product-brief.md) — mission, users, principles, non-goals, and outcomes.
2. [`01-architecture.md`](01-architecture.md) — layers, state/data/display planes, concurrency, and failure containment.
3. [`02-domain-model.md`](02-domain-model.md) — source, range, transform, result, view, selection, provenance, and cache contracts.
4. [`03-gpu-pipeline.md`](03-gpu-pipeline.md) — Metal/WGPU compute and rendering strategy.
5. [`04-view-catalog.md`](04-view-catalog.md) — analytical and expressive visualization catalog.
6. [`05-interaction-model.md`](05-interaction-model.md) — workbench, linked brushing, hypotheses, and accessibility.
7. [`06-plugin-system.md`](06-plugin-system.md) — native/WASM tiers, capabilities, WIT, and declarative scenes.
8. [`07-session-evidence-and-export.md`](07-session-evidence-and-export.md) — sessions, evidence, cache, redaction, and export.
9. [`08-security-privacy.md`](08-security-privacy.md) — threat model and security controls.
10. [`09-observability-testing.md`](09-observability-testing.md) — traces, metrics, corpus, properties, and differential testing.
11. [`10-performance-budgets.md`](10-performance-budgets.md) — latency, memory, source-scale, degradation, and gates.
12. [`11-roadmap.md`](11-roadmap.md) — staged target architecture and de-risk experiments.
13. [`12-outcomes-and-scenarios.md`](12-outcomes-and-scenarios.md) — user journeys and acceptance criteria.
14. [`13-decision-gates.md`](13-decision-gates.md) — assumptions, unresolved product choices, and kill criteria.
15. [`14-dependency-plan.md`](14-dependency-plan.md) — selected and candidate dependency policy.
16. [`15-cli-and-bridge-contract.md`](15-cli-and-bridge-contract.md) — CLI envelope, exit codes, local IPC, and safety.
17. [`16-algorithm-and-artifact-map.md`](16-algorithm-and-artifact-map.md) — complexity, artifact families, planner heuristics, and numerical policy.
18. [`17-poc-session-bundle.md`](17-poc-session-bundle.md) — implemented source-free bundle, integrity, and reattachment boundary.
19. [`18-projection-composition.md`](18-projection-composition.md) — projection, geometry, channels, overlays, comparison, and picking.
20. [`19-signature-knowledge.md`](19-signature-knowledge.md) — strict external signature adapter and evidence semantics.
21. [`20-implementation-status.md`](20-implementation-status.md) — working, experimental, deferred, and release-blocking capabilities.
22. [`21-gui-reference.md`](21-gui-reference.md) — target workbench hierarchy, current baseline, interface ownership, and acceptance invariants.

## Maintainer references

- [`RELEASING.md`](RELEASING.md) — source and macOS binary release checklist.
- [`releases/v0.1.0.md`](releases/v0.1.0.md) — first public preview notes and
  artifact verification.
- [`adr/`](adr/) — architectural decisions and rejected alternatives.
- [`diagrams/`](diagrams/) — editable Mermaid sources.
- [`../CONTRIBUTING.md`](../CONTRIBUTING.md) — contribution workflow and acceptance rules.
- [`../SECURITY.md`](../SECURITY.md) — current guarantees, gaps, and reporting guidance.

Architecture documents may describe a target design that is not implemented.
Where the two differ, implementation status and executable acceptance tests win.
