# AGENTS.md

Guidance for coding agents and contributors working in this repository.

## File roles

- `README.md` holds repository/project description and onboarding context.
- `TODO.md` is the single source of truth for product brief and execution plan.
- `DEVLOG.md` is the chronological execution diary for critical implementation actions.
- `AGENTS.md` contains horizontal instructions for contributors and coding agents.

## Planning protocol (state-of-the-art iterative refinement)

Use a multi-cycle `Critic <-> Builder` loop for any major plan, architecture decision, or cross-file refactor.

- Minimum loop depth: 2 full cycles (`C1 -> B1 -> C2 -> B2`).
- Recommended depth: 3 cycles for security-sensitive or API-contract changes.
- Maximum depth before escalation: 4 cycles, then record open risks and request a decision.

For each `Critic` pass, score and comment on:

- Objective alignment and scope control.
- Security/privacy risks and abuse paths.
- Testability and observability.
- Operational feasibility (build, deploy, rollback).
- Cost/performance impact.

For each `Builder` pass:

- Address every critic finding explicitly (`fixed`, `partially fixed`, `deferred`).
- Preserve a short memory of prior failed approaches to avoid repetition.
- Prefer small verifiable increments over large speculative rewrites.

Refinement stop conditions:

- No unresolved `critical` findings.
- `high` findings either fixed or explicitly accepted with rationale.
- Verification commands exist and are runnable.
- Exit criteria are measurable and unambiguous.

Method references (used to shape this protocol):

- Self-Refine (iterative self-feedback and revision): https://arxiv.org/abs/2303.17651
- Reflexion (verbal feedback memory across trials): https://arxiv.org/abs/2303.11366
- Constitutional AI (critique + revise supervision pattern): https://arxiv.org/abs/2212.08073
- Evals-first workflow for agent reliability: https://www.anthropic.com/engineering/demystifying-evals

## Evaluation-first execution

- Every milestone in `TODO.md` must include explicit verification commands and exit criteria.
- Do not mark a milestone complete without command evidence.
- For risky decisions, add a lightweight spike/prototype command before committing to full implementation.
- When assumptions remain, track them in `TODO.md` as explicit risks with owner + next action.

## Documentation-first coding standard (mandatory)

Assume the primary maintainer has strong `C++`/`Python`/`Pascal` OOP experience and near-zero Rust familiarity.
Write code and comments to be instructional, explicit, and maintainable.

- Add crate-level docs with `//!` for architecture, data flow, and ownership/lifetime expectations.
- Add `///` rustdoc comments to every public type, trait, enum variant, function, method, and constant.
- Add rustdoc to important internal/private items when behavior is non-obvious.
- For non-trivial logic blocks, add inline `//` comments describing intent, invariants, and failure modes.
- Prefer over-documenting to under-documenting for MVP (maximize useful comments and explanations).
- In docs for functions/methods, explain:
  - purpose and context,
  - inputs/outputs,
  - error behavior,
  - side effects (I/O, network, state mutation),
  - security/privacy implications where relevant.
- Include runnable `rustdoc` examples for core APIs when practical.
- Keep comments synchronized with implementation changes; stale comments are treated as defects.
- Follow the canonical template in `docs/RUST_STYLE_GUIDE.md` for crate/module/API structure.

Documentation quality gates for implementation PRs:

- `#![warn(missing_docs)]` (or stricter) enabled for workspace crates.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items` passes.
- `cargo test --workspace --doc` passes once doc examples are introduced.

Versioning rule:

- Repository root `VERSION` is the single source of truth for app version (initially `v0.1.0`).
- UI version display must be derived from this source (directly or build-time generated constant), never hardcoded in multiple locations.
- Any version bump must update `VERSION`, affected tests, and release notes in the same change.

## Execution devlog (mandatory)

During implementation, maintain `DEVLOG.md` as an append-only step-by-step record from scaffolding to final touches.

- Log every critical step (architecture choice, scaffold changes, module integration, security hardening, packaging, release prep).
- Write entries in chronological order with explicit phase mapping (for example `Phase 3`).
- Each entry must include: timestamp, objective, actions performed, files changed, commands run, verification result, and next step.
- Do not defer logging until the end of a milestone; update `DEVLOG.md` continuously during execution.
- If a decision is reversed, add a new corrective entry; never silently rewrite history.

## Documentation hygiene

- Keep roles clean across docs (no product brief duplication in `README.md` or `AGENTS.md`).
- Update `Last updated` in `TODO.md` when plan or product brief content changes.
- Ensure verification commands in `TODO.md` stay runnable and current.
