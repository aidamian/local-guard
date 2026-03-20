# AGENTS.md

Guidance for coding agents and contributors working in this repository.

## File roles

- `README.md` holds repository/project description and onboarding context.
- `TODO.md` is the single source of truth for product brief and execution plan.
- `DEVLOG.md` is the chronological execution diary for critical implementation actions.
- `AGENTS.md` contains horizontal instructions for contributors and coding agents.
- `docs/AGENT_WORKFLOW.md` contains the detailed actor-critic / builder-evaluator-manager execution model.

## Planning protocol (state-of-the-art iterative refinement)

Use a multi-cycle `Manager -> Critic -> Builder/Actor -> Evaluator -> Manager` loop for any major plan, architecture decision, or cross-file refactor.

Roles:

- `Manager`: define objective, constraints, decomposition, verification plan, stop conditions, and accepted risks.
- `Critic`: perform adversarial review for correctness, abuse paths, missing tests, weak observability, and rollout/rollback gaps.
- `Builder/Actor`: implement the smallest verifiable increment that addresses the highest-severity findings first.
- `Evaluator`: gather evidence with deterministic checks first, then rubric/model-based review only where deterministic evidence is insufficient.

Loop depth:

- Minimum loop depth: 2 full cycles (`M0 -> C1 -> B1 -> E1 -> M1 -> C2 -> B2 -> E2 -> M2`).
- Recommended depth: 3 cycles for security-sensitive, privacy-sensitive, or API-contract changes.
- Maximum depth before escalation: 4 cycles, then record open risks and request a decision.

For each `Critic` pass, score and comment on:

- Objective alignment and scope control.
- Security/privacy risks and abuse paths.
- Testability and observability.
- Operational feasibility (build, deploy, rollback).
- Cost/performance impact.

For each `Builder/Actor` pass:

- Address every critic/evaluator finding explicitly (`fixed`, `partially fixed`, `deferred`, `rejected`).
- Preserve a short memory of prior failed approaches to avoid repetition.
- Prefer small verifiable increments over large speculative rewrites.
- Surface new assumptions, side effects, and rollback implications.

For each `Evaluator` pass:

- Run deterministic validators first (`fmt`, `clippy`, tests, builds, schema/doc checks, artifact assertions).
- Separate capability evals from regression evals and promote stable capability checks into regression gates when possible.
- Use rubric/model-based judges only when rules-based checks cannot measure the property directly.
- For ambiguous or high-stakes judgments, use multiple independent evaluator perspectives or panel/jury-style review before acceptance.

Refinement stop conditions:

- No unresolved `critical` findings.
- `high` findings either fixed or explicitly accepted with rationale.
- Verification commands exist and are runnable.
- Exit criteria are measurable and unambiguous.
- The manager has recorded remaining risks, if any, and why another cycle is not justified.

Pattern-selection rule:

- Start with the simplest viable pattern: single builder + evaluator loop for routine work.
- Escalate to sequential workflows when the steps are predictable and staged.
- Escalate to manager-worker or parallel specialist workflows only when the task splits cleanly across files/modules or expertise areas.
- Do not start with complex multi-agent orchestration when a single-agent workflow plus strong evals is sufficient.

Detailed workflow guidance lives in `docs/AGENT_WORKFLOW.md`.

Method references (used to shape this protocol):

- Self-Refine (iterative self-feedback and revision): https://arxiv.org/abs/2303.17651
- Reflexion (verbal feedback memory across trials): https://arxiv.org/abs/2303.11366
- Constitutional AI (critique + revise supervision pattern): https://arxiv.org/abs/2212.08073
- Anthropic, Building Effective AI Agents: https://www.anthropic.com/research/building-effective-agents/
- Anthropic, How we built our multi-agent research system: https://www.anthropic.com/engineering/built-multi-agent-research-system
- Anthropic, Demystifying evals for AI agents: https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents
- LLM-ARC (actor + automated reasoning critic): https://arxiv.org/abs/2406.17663
- ArCHer (hierarchical actor-critic RL for language agents): https://arxiv.org/abs/2402.19446
- Replacing Judges with Juries: https://arxiv.org/abs/2404.18796

## Evaluation-first execution

- Every milestone in `TODO.md` must include explicit verification commands and exit criteria.
- Do not mark a milestone complete without command evidence.
- For risky decisions, add a lightweight spike/prototype command before committing to full implementation.
- When assumptions remain, track them in `TODO.md` as explicit risks with owner + next action.
- Prefer layered evaluation:
  - deterministic gates first,
  - task-local capability/regression evals second,
  - model-based judges only for qualities that cannot be scored mechanically,
  - human escalation when evaluator disagreement remains material.

## Post-modification validation gate (mandatory)

After every code modification (including agent-generated edits), run the following commands before handoff:

- `cargo fmt --all`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace`
- `cargo build --release --target x86_64-pc-windows-gnu`

Any failure must be fixed or explicitly documented with rationale before the task is considered complete.

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
