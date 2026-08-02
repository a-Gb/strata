# ADR 0007: Serializable reducer state plus append-only event journal

- Status: Proposed
- Scope: sessions, undo, evidence, recovery

## Context

The workbench needs undo, crash recovery, source-free sharing, long-running asynchronous results, and immutable evidence records. Directly serializing UI widgets or mutable object graphs would be brittle.

## Decision

Represent user intent as typed commands reduced into serializable session state and append meaningful state transitions to a journal. Store transient GPU/OS objects outside the session. Periodically snapshot state. Seal evidence records and supersede rather than mutate them.

## Consequences

- Reproducible state transitions and better recovery.
- Schema/version migration requires discipline.
- High-frequency camera/hover state should be coalesced rather than journaled verbatim.
- Derived analysis artifacts are referenced by content identity.
