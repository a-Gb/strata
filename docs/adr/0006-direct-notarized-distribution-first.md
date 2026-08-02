# ADR 0006: Direct notarized distribution first

- Status: Proposed
- Scope: packaging and entitlement strategy

## Context

Likely workflows include large disk/firmware images, local bridges to reverse-engineering tools, custom plugin bundles, and possibly later process snapshots. A strict store sandbox may constrain these workflows or split the architecture early.

## Decision

Ship a hardened, signed, notarized direct macOS application first. Maintain least privilege and a sandbox-compatible operating mode where practical. Reassess a store edition after the core workflow is validated.

## Consequences

- More flexibility for professional workflows.
- The project owns update signing and distribution security.
- App Store reach is deferred.
- Privileged features still require separate explicit design and authorization.
