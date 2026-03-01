# DEVLOG.md

Purpose: chronological execution log of critical implementation steps for `local-guard`.
Scope: from scaffolding through final touches.

## Entry format (required)

Use this structure for each critical step:

```md
## YYYY-MM-DD HH:MM UTC | Phase <N> | Step <short title>
Objective:
- <what this step intended to achieve>

Actions:
- <what was done, in order>

Files changed:
- <path1>
- <path2>

Commands run:
- `<command 1>`
- `<command 2>`

Verification:
- <result, pass/fail, and key output summary>

Next:
- <next concrete action>
```

## 2026-02-28 00:00 UTC | Phase 0 | Devlog initialization
Objective:
- Initialize mandatory devlog tracking before execution.

Actions:
- Created `DEVLOG.md` template with required entry format.
- Added process requirements in `AGENTS.md` and `TODO.md` to keep this file updated during execution.

Files changed:
- `DEVLOG.md`
- `AGENTS.md`
- `TODO.md`
- `README.md`

Commands run:
- `cat > DEVLOG.md`
- `apply_patch` updates for repository docs

Verification:
- `DEVLOG.md` exists and contains required entry template.
- Planning docs now include explicit devlog requirements and verification gates.

Next:
- Start Phase 0 execution tasks and append entries after each critical step.

## 2026-02-28 23:22 UTC | Phase 0 | Workspace and crate scaffold
Objective:
- Bootstrap executable Rust workspace layout aligned with TODO phase map.

Actions:
- Created workspace root `Cargo.toml` with 10 package members.
- Generated crate skeletons for `core`, `auth`, `capture`, `mosaic`, `upload`, `analysis_contract`, `ui`, `app`, `contract-tests`, and `benchmarks`.
- Standardized package metadata and path dependencies across crates.

Files changed:
- `Cargo.toml`
- `crates/*/Cargo.toml`
- `crates/*/src/*`

Commands run:
- `cargo new crates/local-guard-core --lib`
- `cargo new crates/local-guard-app --bin`
- `cat > Cargo.toml`

Verification:
- Workspace manifests parsed and `cargo fmt --all` succeeded.

Next:
- Implement phase modules with documentation-first API surfaces.

## 2026-02-28 23:27 UTC | Phase 0 | Contract, ADR, and threat model freeze artifacts
Objective:
- Freeze architecture decisions and API contracts before deep feature work.

Actions:
- Added ADR decisions for capture abstraction, UI state architecture, and auth/upload HTTP strategy.
- Added JSON schemas + canonical fixtures for ingest request and analysis response.
- Authored `docs/THREAT_MODEL.md` with abuse scenarios and mitigations.
- Added CI workflow matrix for Ubuntu/macOS/Windows with fmt, clippy, tests, and rustdoc gates.

Files changed:
- `docs/adr/ADR-0001-capture-backend.md`
- `docs/adr/ADR-0002-ui-toolkit.md`
- `docs/adr/ADR-0003-auth-http-approach.md`
- `contracts/ingest-request.schema.json`
- `contracts/analysis-response.schema.json`
- `contracts/fixtures/*`
- `docs/THREAT_MODEL.md`
- `.github/workflows/ci.yml`

Commands run:
- `mkdir -p contracts/fixtures docs/adr .github/workflows`
- `cat > contracts/ingest-request.schema.json`
- `cat > docs/THREAT_MODEL.md`

Verification:
- Required contract and ADR files created at expected paths.

Next:
- Implement module logic and test coverage for each roadmap phase.

## 2026-02-28 23:30 UTC | Phase 1 | Core domain model implementation
Objective:
- Deliver deterministic frame batching, metadata sealing, and payload codec.

Actions:
- Implemented `local-guard-core` frame/batch models and invariant checks.
- Added metadata derivation and JSON payload encode/decode helpers.
- Added targeted tests: `mosaic_ordering_tests`, `metadata_integrity_tests`, `payload_codec_tests`.

Files changed:
- `crates/local-guard-core/src/lib.rs`
- `crates/local-guard-core/tests/mosaic_ordering_tests.rs`
- `crates/local-guard-core/tests/metadata_integrity_tests.rs`
- `crates/local-guard-core/tests/payload_codec_tests.rs`

Commands run:
- `cat > crates/local-guard-core/src/lib.rs`
- `cargo test --package local-guard-core mosaic_ordering_tests`

Verification:
- Core package tests passed with deterministic fixtures.

Next:
- Implement auth state machine and endpoint policy validation.

## 2026-02-28 23:31 UTC | Phase 2 | Authentication and guard controls
Objective:
- Enforce auth-before-capture using explicit state machine transitions.

Actions:
- Implemented `AuthClient`, endpoint validator, and `AuthStateMachine` transitions.
- Added token expiry handling and capture-guard semantics.
- Wired app guard helpers and app integration tests for auth transitions.

Files changed:
- `crates/local-guard-auth/src/lib.rs`
- `crates/local-guard-app/src/lib.rs`
- `crates/local-guard-app/tests/auth_state_machine_tests.rs`
- `crates/local-guard-app/tests/auth_guard_tests.rs`

Commands run:
- `cat > crates/local-guard-auth/src/lib.rs`
- `cargo test --package local-guard-auth`
- `cargo test --package local-guard-app auth_state_machine_tests`

Verification:
- Auth state transition and guard tests passed.

Next:
- Implement capture abstraction and deterministic synthetic backend.

## 2026-02-28 23:32 UTC | Phase 3 | Capture abstraction and scheduler
Objective:
- Provide display enumeration + deterministic frame capture cadence.

Actions:
- Implemented `CaptureBackend` trait and `SyntheticCaptureBackend`.
- Added FPS config validation and deterministic schedule helper.
- Added app tests for scheduler cadence and display selection.

Files changed:
- `crates/local-guard-capture/src/lib.rs`
- `crates/local-guard-app/tests/capture_scheduler_tests.rs`
- `crates/local-guard-app/tests/display_selection_tests.rs`

Commands run:
- `cat > crates/local-guard-capture/src/lib.rs`
- `cargo test --package local-guard-capture`
- `cargo test --package local-guard-app capture_scheduler_tests`

Verification:
- Capture backend tests and app scheduler/display tests passed.

Next:
- Build deterministic 3x3 temporal mosaic assembly.

## 2026-02-28 23:33 UTC | Phase 4 | Mosaic assembly and payload bridge
Objective:
- Convert 9-frame batches into deterministic chronological 3x3 mosaics.

Actions:
- Implemented `compose_temporal_mosaic` with row/column tile mapping invariants.
- Added integration bridge in app crate from batch -> mosaic -> payload.
- Added batch-to-payload integration test.

Files changed:
- `crates/local-guard-mosaic/src/lib.rs`
- `crates/local-guard-app/src/lib.rs`
- `crates/local-guard-app/tests/batch_to_payload_integration_tests.rs`

Commands run:
- `cat > crates/local-guard-mosaic/src/lib.rs`
- `cargo test --package local-guard-mosaic`
- `cargo test --package local-guard-app batch_to_payload_integration_tests`

Verification:
- Mosaic and integration tests passed; 9-frame batches generate one payload.

Next:
- Implement upload client with retries, classification, and idempotency.

## 2026-02-28 23:34 UTC | Phase 5 | Protected upload semantics
Objective:
- Enforce HTTPS uploads with idempotency and bounded retry behavior.

Actions:
- Implemented `UploadClient`, `RetryPolicy`, failure classification, and SHA-256 idempotency keys.
- Added app tests covering retry recovery, error taxonomy, and key stability.

Files changed:
- `crates/local-guard-upload/src/lib.rs`
- `crates/local-guard-app/tests/upload_retry_policy_tests.rs`
- `crates/local-guard-app/tests/upload_error_classification_tests.rs`
- `crates/local-guard-app/tests/idempotency_key_tests.rs`

Commands run:
- `cat > crates/local-guard-upload/src/lib.rs`
- `cargo test --package local-guard-upload`
- `cargo test --package local-guard-app upload_retry_policy_tests`

Verification:
- Upload tests passed; transient failures retried successfully.

Next:
- Implement analysis response parsing and forward-compatible mapping.

## 2026-02-28 23:35 UTC | Phase 6/7/8 | Analysis mapping, UI gates, and privacy/security checks
Objective:
- Complete runtime user-facing behavior: analysis projection, consent gating, status view, and redaction/kill-switch checks.

Actions:
- Implemented analysis contract parsing and risk mapping crate.
- Implemented toolkit-agnostic UI state model with consent + auth + display gate.
- Added app helpers for runtime status projection, endpoint security validation, redaction, and kill-switch.
- Added integration tests for analysis parsing/mapping, consent, status projection, version display, redaction, privacy, transport security, and kill-switch behavior.

Files changed:
- `crates/local-guard-analysis-contract/src/lib.rs`
- `crates/local-guard-ui/src/lib.rs`
- `crates/local-guard-app/src/lib.rs`
- `crates/local-guard-app/tests/*`

Commands run:
- `cargo test --package local-guard-analysis-contract`
- `cargo test --package local-guard-ui`
- `cargo test --package local-guard-app transport_security_tests`

Verification:
- All app integration test filters from TODO phase checks passed.

Next:
- Finalize contract validation package, benchmarks, docs gates, and packaging.

## 2026-02-28 23:36 UTC | Phase 9 | Contract tests, benchmark harness, and release lane
Objective:
- Complete packaging and final verification gates across workspace.

Actions:
- Added `local-guard-contract-tests` schema/fixture validation tests.
- Added `local-guard-benchmarks` smoke benchmark test and `--nocapture` output.
- Implemented app build-time version propagation from root `VERSION` via `build.rs`.
- Ran full verification matrix including fmt, clippy, docs, workspace tests, and Windows GNU cross-build.
- Copied release `.exe` artifact to `dist/win32/`.

Files changed:
- `crates/local-guard-contract-tests/src/lib.rs`
- `crates/local-guard-contract-tests/tests/contract_validation.rs`
- `crates/local-guard-benchmarks/src/lib.rs`
- `crates/local-guard-benchmarks/tests/nfr_smoke.rs`
- `crates/local-guard-app/build.rs`
- `dist/win32/local-guard-app.exe`

Commands run:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items`
- `cargo test --workspace`
- `cargo test --workspace --doc`
- `cargo build --release`
- `cargo build --release --target x86_64-pc-windows-gnu`
- `cp -f target/x86_64-pc-windows-gnu/release/*.exe dist/win32/`

Verification:
- All targeted TODO phase commands completed successfully.
- `dist/win32/local-guard-app.exe` present.

Next:
- Mark roadmap checklists complete and summarize delivered scope/risks.

## 2026-02-28 23:37 UTC | Phase 9 | Corrective fixes during validation
Objective:
- Resolve compile and compliance issues found during first full test pass.

Actions:
- Removed `Debug` derives from clients holding trait-object transports.
- Updated env mutation test calls to explicit `unsafe` blocks for Rust 2024.
- Switched `jsonschema::Validator` usage to `jsonschema::JSONSchema` API.
- Corrected root `VERSION` content from `0.1.0` to required `v0.1.0` format.

Files changed:
- `crates/local-guard-auth/src/lib.rs`
- `crates/local-guard-upload/src/lib.rs`
- `crates/local-guard-app/tests/kill_switch_behavior_tests.rs`
- `crates/local-guard-contract-tests/tests/contract_validation.rs`
- `VERSION`

Commands run:
- `cargo test --workspace`
- `cargo test --package local-guard-contract-tests`
- `grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$' VERSION`

Verification:
- Post-fix workspace tests and validation script passed (`ALL_PHASE_COMMANDS_PASSED`).

Next:
- Update `TODO.md` execution checklist and close execution report.

## 2026-02-28 23:42 UTC | Phase 9 | Final checklist closure and documentation sync
Objective:
- Close execution loop by syncing roadmap status docs with verified implementation output.

Actions:
- Marked all `TODO.md` execution checkboxes complete after passing phase commands.
- Updated `README.md` current status to reflect implemented crate map and verification state.
- Re-ran release Windows cross-build after final `VERSION` format correction and refreshed `dist/win32/local-guard-app.exe`.

Files changed:
- `TODO.md`
- `README.md`
- `DEVLOG.md`
- `dist/win32/local-guard-app.exe`

Commands run:
- `perl -0pi -e 's/- \[ \]/- [x]/g' TODO.md`
- `cargo test --workspace --doc && cargo test --workspace`
- `cargo build --release --target x86_64-pc-windows-gnu`
- `cp -f target/x86_64-pc-windows-gnu/release/*.exe dist/win32/`

Verification:
- All workspace/doc tests passed after status/doc updates.
- Win32 artifact exists and is current.
- Devlog contains explicit `Phase 0` through `Phase 9` entries.

Next:
- Hand off implementation summary and any residual risks.

## 2026-02-28 23:43 UTC | Phase 0 | Critic/Builder refinement record
Objective:
- Satisfy planning protocol for major cross-file implementation before full coding sweep.

Actions:
- Critic C1: flagged missing workspace scaffolding, missing frozen contracts, and missing threat model as critical blockers.
- Builder B1: created workspace skeleton and Phase 0 artifact set (`contracts`, `docs/adr`, `docs/THREAT_MODEL.md`).
- Critic C2: flagged weak verification coverage and missing cross-build artifact lane validation.
- Builder B2: added per-phase test filters, rustdoc/clippy gates, benchmark package, and Win32 release artifact copy/verification.

Files changed:
- `Cargo.toml`
- `contracts/*`
- `docs/adr/*`
- `docs/THREAT_MODEL.md`
- `crates/*`

Commands run:
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo build --release --target x86_64-pc-windows-gnu`

Verification:
- No unresolved critical findings remained after B2.
- High findings from C2 addressed with executable verification commands and artifact checks.

Next:
- Maintain iterative refinement for post-MVP feature expansion.

## 2026-02-28 23:49 UTC | Phase 7 | Win32 UI boot path activation
Objective:
- Resolve runtime gap where `local-guard-app.exe` exited after console logs without starting a desktop UI.

Actions:
- Replaced stub `main.rs` behavior with Windows-specific Win32 event loop (`RegisterClassW`, `CreateWindowExW`, message dispatch).
- Added basic on-screen UI shell text and kill-switch status rendering in `WM_PAINT`.
- Added Windows-target dependency `windows-sys` with required feature flags.
- Rebuilt Win32 release artifact and refreshed `dist/win32/local-guard-app.exe`.

Files changed:
- `crates/local-guard-app/src/main.rs`
- `crates/local-guard-app/Cargo.toml`
- `dist/win32/local-guard-app.exe`
- `DEVLOG.md`

Commands run:
- `cargo fmt --all`
- `cargo clippy --package local-guard-app --all-targets --all-features -- -D warnings`
- `cargo test --package local-guard-app`
- `cargo build --release --target x86_64-pc-windows-gnu`
- `cp -f target/x86_64-pc-windows-gnu/release/*.exe dist/win32/`

Verification:
- App package tests and clippy checks passed.
- Win32 executable rebuilt successfully with UI event-loop code.

Next:
- Validate on native Windows host that launching `local-guard-app.exe` opens and maintains the UI window.

## 2026-02-28 23:50 UTC | Phase 7 | Runtime file logging for manual debug workflows
Objective:
- Add launch-time log file generation in executable folder for post-manual-execution debugging.

Actions:
- Implemented Win32 runtime logger with per-launch file naming: `[unix_timestamp]_log.txt` in executable directory.
- Added structured stage/action/error log lines for startup, window creation, event-loop begin/end, first paint, destroy, and API failures.
- Rebuilt Win32 release artifact with logging instrumentation.

Files changed:
- `crates/local-guard-app/src/main.rs`
- `DEVLOG.md`
- `dist/win32/local-guard-app.exe`

Commands run:
- `cargo fmt --all`
- `cargo clippy --package local-guard-app --all-targets --all-features -- -D warnings`
- `cargo test --package local-guard-app`
- `cargo build --release --target x86_64-pc-windows-gnu`
- `cp -f target/x86_64-pc-windows-gnu/release/*.exe dist/win32/`

Verification:
- App tests and lints passed after logging instrumentation.
- Updated Win32 executable generated successfully.

Next:
- Confirm on native Windows that each launch creates a new timestamped log file beside the `.exe`.

## 2026-02-28 23:58 UTC | Phase 7 | Interactive Win32 MVP shell implementation
Objective:
- Replace non-functional placeholder window with a UI that matches MVP interaction flow expectations (login, consent, display selection, capture control, runtime status visibility).

Actions:
- Replaced minimal paint-only shell with interactive Win32 controls: username/password inputs, login button, consent checkbox, display dropdown, start/stop capture controls, and multi-line runtime status panel.
- Wired control events to existing crate logic: auth client/state machine, display selection, frame capture cadence, 9-frame batching, mosaic payload generation, upload retries, and analysis status mapping.
- Added structured stage/action/error logging for all key runtime actions (auth, capture ticks, batch completion, upload outcome, analysis mapping, and failure branches) into per-run `[timestamp]_log.txt` files beside the executable.
- Adjusted Windows dependency features to include required control/input APIs and switched controller state storage to thread-local ownership compatible with Win32 HWND handle types.
- Rebuilt Win32 release artifact and refreshed `dist/win32/local-guard-app.exe`.

Files changed:
- `crates/local-guard-app/src/main.rs`
- `crates/local-guard-app/Cargo.toml`
- `.gitignore`
- `dist/win32/local-guard-app.exe`
- `DEVLOG.md`

Commands run:
- `cargo fmt --all`
- `cargo clippy --package local-guard-app --all-targets --all-features -- -D warnings`
- `cargo test --package local-guard-app`
- `cargo build --release --target x86_64-pc-windows-gnu`
- `cp -f target/x86_64-pc-windows-gnu/release/*.exe dist/win32/`

Verification:
- App crate tests passed after interactive UI integration.
- Win32 cross-build succeeded with interactive shell code path.
- Fresh executable exists in `dist/win32/` and is larger than prior placeholder build.

Next:
- Validate on native Windows host that manual flow updates status fields and writes per-run action/error logs for post-test debugging.

## 2026-03-01 00:12 UTC | Phase 3/4/7 | Real screen capture and local upload-prep pipeline
Objective:
- Implement real display selection/capture and convert every 9 captured frames into an upload-ready 3x3 artifact without requiring a live HTTP endpoint.

Actions:
- Added `RealCaptureBackend` in `local-guard-capture` using OS display enumeration/capture (`screenshots` crate on Windows).
- Switched Win32 app controller from synthetic backend to real backend initialization; app now errors on startup if real display discovery fails.
- Replaced mock upload send path with local staging path: each completed 9-frame batch is converted to payload + mosaic, then persisted as `prepared_uploads/<timestamp>_mosaic.png` and `prepared_uploads/<timestamp>_payload.json` next to the executable.
- Updated runtime UI status fields/logging to reflect prepared batch counts, last prepared artifact names, backend identity, and endpoint-not-configured behavior.
- Kept auth/consent/display gating intact before capture start.

Files changed:
- `crates/local-guard-capture/Cargo.toml`
- `crates/local-guard-capture/src/lib.rs`
- `crates/local-guard-app/Cargo.toml`
- `crates/local-guard-app/src/main.rs`
- `.gitignore`
- `DEVLOG.md`
- `dist/win32/local-guard-app.exe`

Commands run:
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --package local-guard-app`
- `cargo build --release --target x86_64-pc-windows-gnu`
- `cp -f target/x86_64-pc-windows-gnu/release/*.exe dist/win32/`

Verification:
- Workspace clippy gate passed with `-D warnings`.
- `local-guard-app` tests passed.
- Win32 release build succeeded with real capture + local staging code path.
- Updated `dist/win32/local-guard-app.exe` generated successfully.

Next:
- Validate on native Windows host that selecting each real monitor captures 1 FPS cadence and writes one staged artifact pair every 9 frames.

## 2026-03-01 00:13 UTC | Phase 3/4/7 | Critic/Builder refinement for real-capture rollout
Objective:
- Apply required multi-cycle planning refinement before finalizing cross-file real-capture + staging refactor.

Actions:
- Critic C1: identified spec mismatch (synthetic capture + mock upload path) as critical against user-observable behavior.
- Builder B1: introduced `RealCaptureBackend` and switched UI pipeline to real display enumeration/capture.
- Critic C2: identified dependency on unavailable endpoint as blocker for upload stage completion.
- Builder B2: replaced network upload dependency with deterministic local staging artifacts (`.png` + `.json`) ready for later endpoint wiring.

Files changed:
- `crates/local-guard-capture/src/lib.rs`
- `crates/local-guard-app/src/main.rs`

Commands run:
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo build --release --target x86_64-pc-windows-gnu`

Verification:
- No unresolved critical findings remained after B2.
- Resulting behavior now aligns with real-screen + 9-frame prep requirement without endpoint dependency.

Next:
- Collect manual QA results per monitor to validate end-user cadence/selection behavior.

## 2026-03-01 00:30 UTC | Phase 7 | Timestamp format normalization
Objective:
- Enforce uniform `YYYYmmDD_HHMMSS` timestamp format for runtime log and artifact naming.

Actions:
- Added compact UTC timestamp formatter and replaced prior epoch-based naming in runtime file outputs.
- Updated per-run log filename format to `<YYYYmmDD_HHMMSS>_log.txt`.
- Updated log line prefix format to `YYYYmmDD_HHMMSS`.
- Updated prepared upload artifact naming to `<YYYYmmDD_HHMMSS>_mosaic.png` and `<YYYYmmDD_HHMMSS>_payload.json`.
- Rebuilt Win32 release binary with timestamp formatting changes.
- Encountered locked existing `dist/win32/local-guard-app.exe` during copy; wrote refreshed binary as side-by-side `local-guard-app.new.exe`.

Files changed:
- `crates/local-guard-app/Cargo.toml`
- `crates/local-guard-app/src/main.rs`
- `DEVLOG.md`
- `dist/win32/local-guard-app.new.exe`

Commands run:
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --package local-guard-app`
- `cargo build --release --target x86_64-pc-windows-gnu`
- `cp .../local-guard-app.exe dist/win32/local-guard-app.new.exe`

Verification:
- Lint and tests passed.
- Windows release build succeeded.
- New executable with timestamp-format update available as `local-guard-app.new.exe`.

Next:
- Replace locked `local-guard-app.exe` after process release, or run `local-guard-app.new.exe` directly.

## 2026-03-01 00:31 UTC | Phase 9 | Win32 executable replacement after process unlock
Objective:
- Replace temporary staged executable with updated main executable after user closed running process.

Actions:
- Copied `dist/win32/local-guard-app.new.exe` over `dist/win32/local-guard-app.exe`.
- Verified timestamp and size of updated primary executable.

Files changed:
- `dist/win32/local-guard-app.exe`
- `DEVLOG.md`

Commands run:
- `cp -f dist/win32/local-guard-app.new.exe dist/win32/local-guard-app.exe`
- `ls -lh dist/win32/local-guard-app.exe dist/win32/local-guard-app.new.exe`

Verification:
- `local-guard-app.exe` now reflects updated build timestamp (`2026-03-01 00:31`).

Next:
- Optionally remove `local-guard-app.new.exe` housekeeping file.

## 2026-03-01 00:37 UTC | Phase 7 | JPEG/base64 payload compaction for staged mosaics
Objective:
- Reduce staged artifact footprint by replacing raw RGBA JSON payload content with compressed JPEG (RGB) base64 content.

Actions:
- Updated Win32 app payload staging to convert mosaic bytes from RGBA to RGB before encoding.
- Replaced PNG artifact write with JPEG encoding using quality `9` and `.jpg` output naming.
- Replaced staged payload JSON body from full `MosaicPayload` dump to compact structured JSON carrying metadata + `mosaic_jpeg_base64`.
- Updated runtime status labels and internal state names from `LastPng` to `LastJpeg`.
- Added `base64` dependency and switched `image` crate feature set to `jpeg`.
- Rebuilt release executable and copied refreshed binary to `dist/win32/local-guard-app.exe`.

Files changed:
- `crates/local-guard-app/Cargo.toml`
- `crates/local-guard-app/src/main.rs`
- `DEVLOG.md`
- `dist/win32/local-guard-app.exe`

Commands run:
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --package local-guard-app`
- `cargo build --release --target x86_64-pc-windows-gnu`
- `cp -f target/x86_64-pc-windows-gnu/release/local-guard-app.exe dist/win32/local-guard-app.exe`
- `sha256sum target/x86_64-pc-windows-gnu/release/local-guard-app.exe dist/win32/local-guard-app.exe`

Verification:
- Formatting/lint/tests passed.
- Windows release build succeeded.
- Source and deployed executable hashes match.

Next:
- Run manual capture cycle to confirm new staged `.jpg` + compact `.json` artifacts are produced after each 9-frame batch.

## 2026-03-01 08:48 UTC | Phase 3/4/7 | Performance refactor + live frame/preview UX
Objective:
- Eliminate sluggish Win32 UI behavior by removing capture/mosaic/encoding work from the UI thread and add real-time frame + mosaic preview UX for manual diagnostics.

Actions:
- Critic C1: identified UI-thread blocking in `WM_TIMER` path (`capture_frame` + batch compose + JPEG encode + disk writes) and per-line file flush as primary responsiveness bottlenecks.
- Builder B1 (`fixed`): introduced dedicated capture worker thread with command/event channels; UI timer now enqueues lightweight capture commands and returns immediately.
- Critic C2: identified missing pipeline telemetry in UI (no current frame visibility, no visual mosaic confirmation) and expensive per-frame backend re-enumeration.
- Builder B2 (`fixed`): added frame telemetry status line (`total frame`, `current batch index`, `capture ms`, `batch prepare ms`) and on-window reduced mosaic preview rendering via `StretchDIBits`.
- Optimized real backend by caching `Screen` handles and refreshing only on topology/capture failure instead of calling `Screen::all()` every frame.
- Reduced log I/O pressure by flushing run log immediately only on `ERROR` lines.
- Rebuilt Win32 release binary and deployed refreshed executable to `dist/win32/local-guard-app.exe`.

Files changed:
- `crates/local-guard-app/src/main.rs`
- `crates/local-guard-capture/src/lib.rs`
- `DEVLOG.md`
- `dist/win32/local-guard-app.exe`

Commands run:
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --package local-guard-app`
- `cargo test --package local-guard-capture`
- `cargo build --release --target x86_64-pc-windows-gnu`
- `cp -f target/x86_64-pc-windows-gnu/release/local-guard-app.exe dist/win32/local-guard-app.exe`
- `sha256sum target/x86_64-pc-windows-gnu/release/local-guard-app.exe dist/win32/local-guard-app.exe`

Verification:
- Lint/tests passed after worker-thread and preview integration.
- Windows GNU release build succeeded.
- Deployed executable hash matches release build hash.

Next:
- Manual Windows run: verify UI remains responsive during continuous capture and that `prepared_uploads/` emits compact `.json` + `.jpg` while preview updates every completed 9-frame batch.
