# THREAT_MODEL.md

Last updated: 2026-02-28

## Scope

MVP desktop capture client (`local-guard`) and its interactions with auth + ingest APIs.

## Assets

- User credentials entered at login.
- Short-lived session tokens.
- Raw frame data in memory.
- Mosaic payloads + metadata in transit.

## Trust boundaries

1. Local UI and process memory.
2. Network boundary to auth endpoint (`/r1/cstore-auth`).
3. Network boundary to protected ingest API.
4. Logging/telemetry sinks.

## Abuse/misuse scenarios and mitigations

| ID | Scenario | Risk | Mitigation |
|---|---|---|---|
| TM-01 | Capture starts before login | Unauthorized data collection | Auth state machine + guard checks before capture |
| TM-02 | Capture starts without explicit consent | Privacy violation | Consent gate in UI state model |
| TM-03 | Token leaked in logs | Session hijack | Redaction helpers + tests for token/password markers |
| TM-04 | Raw frame written to disk | Sensitive data persistence | MVP policy: in-memory batch only, no raw frame persistence |
| TM-05 | MITM on API calls | Data tampering/exfiltration | Enforce HTTPS endpoint validation |
| TM-06 | Retry storm during outage | Resource exhaustion/noisy loops | Capped exponential backoff + failure classification |
| TM-07 | Unknown analysis category crashes client | Availability loss | Forward-compatible category handling |
| TM-08 | Runtime emergency stop needed | Operational control gap | `LOCAL_GUARD_CAPTURE_ENABLED` kill-switch |

## Residual risks (MVP)

- No certificate pinning in MVP.
- In-memory queue only; pending payloads can be lost on process crash.
- GUI shell hardening deferred to v1.1.

## Next actions

1. Add periodic abuse-case review in release checklist.
2. Expand transport hardening (pinning/allowlist) in v1.1.
3. Add structured security event telemetry schema.
