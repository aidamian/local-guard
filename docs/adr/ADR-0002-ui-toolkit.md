# ADR-0002: UI state-first architecture

- Status: Accepted
- Date: 2026-02-28

## Context

Desktop shell technology may change between MVP iterations, but runtime behavior must stay stable.

## Decision

Freeze a toolkit-agnostic state model in `local-guard-ui` for auth, consent, capture, network, upload, and analysis statuses.

- Current MVP exposes pure Rust state and reducers.
- Shell rendering layer (tauri/egui/native host) remains replaceable as long as it consumes `UiState`.

## Consequences

- UI behavior is testable without GUI dependencies.
- Future toolkit migration has lower risk.
- Requires clear mapping between backend events and UI state transitions.
