# ADR 0001: Native AppKit host with Rust-owned core

- Status: Proposed
- Scope: macOS application shell

## Context

The product needs native document/file access, menus, keyboard handling, drag/drop, high-DPI display behavior, accessibility, multiple windows, and future external-tool integration. The analytical engine and visualization canvas should remain Rust-owned and testable outside the GUI.

## Decision

Use AppKit/Foundation through `objc2` for application lifecycle and native integration. Host a `wgpu` Metal surface for the primary canvas. Evaluate a lightweight Rust UI layer for panels and docking, but keep session/domain state independent of it.

## Consequences

- Better macOS integration and a clear main-thread boundary.
- Some audited Objective-C FFI/unsafe code will exist in a narrow outer crate.
- Cross-platform UI is deferred, while the engine remains portable.
- Accessibility for custom canvas content must be designed explicitly.

## Rejected as default

- WebView/Tauri: convenient UI, but introduces a process/serialization boundary around the highest-frequency canvas interactions.
- Pure `winit` host: faster spike, weaker native document and AppKit ownership story.
- Fully custom UI: excessive cost for text, accessibility, menus, and ordinary controls.
