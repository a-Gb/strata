# ADR 0004: WebAssembly Component Model for external plugins

- Status: Proposed
- Scope: third-party extension

## Context

The application handles hostile inputs and sensitive source bytes. A conventional native dynamic-library plugin API grants excessive process authority and creates ABI/versioning risk.

## Decision

Use Wasmtime and WIT-defined WebAssembly components as the default external plugin model. Plugins receive capability handles, bounded range reads, quotas, and a declarative result/scene API. First-party plugins may remain native workspace crates.

## Consequences

- External plugins are portable and containable.
- High-frequency or GPU-native plugins may need host-provided primitives rather than arbitrary code.
- Runtime footprint and call overhead require measurement.
- WIT interfaces must be carefully versioned.

## Rejected

- In-process `cdylib` plugins: broad authority, crash propagation, unstable Rust ABI.
- Scripting language embedded with ambient filesystem access: easier authoring but weaker capability boundary.
- Remote plugins: violates local-first and source privacy defaults.
