# ADR 0005: Machine learning outside the critical evidence path

- Status: Accepted for scaffold
- Scope: classification, retrieval, anomaly ranking

## Context

Visual fingerprints can support similarity retrieval and anomaly prioritization, but they are fragile under packing, padding, recompilation, reordering, and adversarial perturbation. Opaque classifiers can produce persuasive but weak conclusions.

## Decision

Build deterministic views, features, and provenance first. Optional local ML may rerank corpus results or prioritize anomalies, but every score must link to deterministic supporting features, model/version identity, and known uncertainty. No verdict depends solely on ML.

## Consequences

- Initial product remains explainable and useful without a model.
- ML integration is easier to benchmark against a baseline.
- Some automatic classification features arrive later.

## Rejected

- Image classifier as the primary file-type/malware engine.
- Cloud embedding service.
- Training on user data by default.
