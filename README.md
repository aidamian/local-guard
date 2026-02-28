# local-guard

Cross-platform Rust desktop app (macOS + Windows) for controlled screen capture and secure upload of compact 3x3 time mosaics.

## Scope

- `README.md`: repository and project overview.
- `TODO.md`: product brief, execution plan, milestones, and verification gates.
- `AGENTS.md`: horizontal working instructions for agents/contributors.

## Development Environment

This repo is configured to use the maintained Microsoft Rust Dev Container base image and install Windows GNU cross-compile support in `postCreateCommand`.

- Base image: `mcr.microsoft.com/devcontainers/rust:1-bookworm`
- Extra packages: `mingw-w64`, `nodejs`, `npm`
- Rust target: `x86_64-pc-windows-gnu`
- Codex CLI: installed globally via `@openai/codex`

Open in VS Code Dev Containers and run:

```bash
codex --version
cargo build --release --target x86_64-pc-windows-gnu
```

Deterministic linker settings for the Windows GNU target are defined in `.cargo/config.toml`.

## Suggested Runtime Configuration

Use environment variables (or secure config storage) for endpoints and runtime behavior:

- `LOCAL_GUARD_AUTH_URL` (auth API endpoint, e.g. `.../r1/cstore-auth`)
- `LOCAL_GUARD_INGEST_URL` (mosaic ingestion endpoint)
- `LOCAL_GUARD_CAPTURE_FPS` (default `1`)
- `LOCAL_GUARD_BATCH_SIZE` (default `9`)

Never hardcode credentials or long-lived tokens.

## Current Repository Status

This repository currently contains bootstrap project documentation and devcontainer setup. Application code scaffold is the next step, guided by `TODO.md`.
