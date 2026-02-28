# local-guard

Rust desktop agent (Windows + macOS) for controlled screen capture and secure delivery of temporal mosaics to a protected edge-cloud analysis API.

## Final objective

`local-guard` captures an operator-selected display, builds 3x3 time mosaics (9 frames), and uploads them to a protected API where multiple models analyze the image for:

- cybersecurity-relevant threats,
- social-engineering indicators,
- session-level risk signals for downstream response workflows.

Client behavior is capture-and-deliver: no client-side censorship or content mutation. Security and policy enforcement happen at the protected API boundary and analysis layer.

## Scope map

- `README.md`: repository overview and operating guidance.
- `TODO.md`: product brief + execution roadmap (single source of execution state).
- `AGENTS.md`: contributor/agent operating protocol.

## Development environment

This repo uses a Rust devcontainer and includes Windows GNU cross-compilation support.

- Base image: `mcr.microsoft.com/devcontainers/rust:1-bookworm`
- Extra packages: `mingw-w64`, `nodejs`, `npm`
- Rust target: `x86_64-pc-windows-gnu`
- Codex CLI: installed globally via `@openai/codex`

Quick check:

```bash
codex --version
cargo build --release --target x86_64-pc-windows-gnu
```

Deterministic linker settings for cross-builds are in `.cargo/config.toml`.

## Runtime configuration

Use environment variables (or secure OS config storage):

- `LOCAL_GUARD_AUTH_URL` (auth endpoint, e.g. `.../r1/cstore-auth`)
- `LOCAL_GUARD_INGEST_URL` (protected ingest endpoint)
- `LOCAL_GUARD_CAPTURE_FPS` (default `1`)
- `LOCAL_GUARD_BATCH_SIZE` (default `9`)

Do not hardcode credentials, API keys, or long-lived tokens.

## Windows manual test artifacts

A dedicated folder is reserved for manually testable Windows builds from inside the devcontainer:

- Output folder: `dist/win32/`
- Build + copy command:

```bash
cargo build --release --target x86_64-pc-windows-gnu
mkdir -p dist/win32
cp -f target/x86_64-pc-windows-gnu/release/*.exe dist/win32/
ls -lh dist/win32
```

## Current status

The repository currently contains project documentation and environment bootstrap. Implementation should follow the execution roadmap in `TODO.md`.
