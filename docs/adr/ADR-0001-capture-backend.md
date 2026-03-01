# ADR-0001: Capture backend abstraction

- Status: Accepted
- Date: 2026-02-28

## Context

V1 needs deterministic CI tests and a portable path to platform-specific capture implementations.

## Decision

Use a trait-based capture backend in `local-guard-capture`:
- `CaptureBackend` for enumeration and frame capture.
- `SyntheticCaptureBackend` as default deterministic implementation for tests.
- Real OS backends will implement the same trait in v1.x.

## Consequences

- Enables deterministic test coverage without native capture dependencies.
- Keeps app orchestration backend-agnostic.
- Requires strict frame-shape invariants enforced in shared domain model.
