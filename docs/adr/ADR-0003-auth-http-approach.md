# ADR-0003: Auth and ingest HTTP contract strategy

- Status: Accepted
- Date: 2026-02-28

## Context

MVP must enforce auth-before-capture and protected HTTPS ingest with stable schemas.

## Decision

- Auth endpoint remains `.../r1/cstore-auth` and must be HTTPS.
- Ingest endpoint must be HTTPS.
- Request/response schemas are frozen in `contracts/*.schema.json` with fixtures in `contracts/fixtures/`.
- Retry policy: capped exponential backoff + jitter, retries only for transient classes.

## Consequences

- Security posture is explicit and testable.
- Contract drift can be caught through fixture validation tests.
- Any endpoint contract migration requires a new ADR and versioned schema update.
