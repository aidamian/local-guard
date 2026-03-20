# AGENT_WORKFLOW.md

Detailed execution protocol for contributors and coding agents performing major planning, architecture, security, or cross-file implementation work in this repository.

Use this guide together with `AGENTS.md`:

- `AGENTS.md` defines the mandatory repo rules.
- `docs/AGENT_WORKFLOW.md` explains the deeper multi-cycle actor-critic / builder-evaluator-manager operating model behind those rules.

## Purpose

This repo uses iterative refinement on purpose:

- major changes are easy to get "mostly right" while still missing security, observability, rollback, or evaluation gaps;
- coding agents benefit from explicit roles and stop conditions instead of free-form self-reflection;
- deterministic evidence must dominate acceptance decisions whenever the task allows it.

The default operating model is:

`Manager -> Critic -> Builder/Actor -> Evaluator -> Manager`

Repeat for multiple cycles until stop conditions are met.

## Role definitions

### `Manager`

Owns task framing and acceptance:

- define the objective, non-goals, constraints, and success metrics;
- choose the simplest architecture that can satisfy the task;
- define verification commands before implementation starts;
- decide whether the work should stay single-agent, use a structured workflow, or escalate to manager-worker orchestration;
- maintain short-term memory of failed approaches, accepted risks, and open questions;
- stop the loop only when evidence is sufficient.

### `Critic`

Acts as an adversarial reviewer and pre-mortem / post-mortem analyst:

- search for correctness gaps and requirement drift;
- identify privacy, security, and abuse paths;
- challenge missing tests, weak observability, and weak rollback plans;
- score findings by severity so the manager can decide whether another cycle is mandatory.

### `Builder` / `Actor`

Produces the next smallest verifiable improvement:

- implement the current best approach, not the largest rewrite;
- respond to every prior critic/evaluator finding explicitly as `fixed`, `partially fixed`, `deferred`, or `rejected`;
- preserve memory of prior failed attempts to avoid oscillation;
- expose assumptions and tradeoffs instead of hiding them.

### `Evaluator`

Produces evidence, not vibes:

- run deterministic checks first;
- use rubric-based or model-based review only when deterministic checks cannot measure the property directly;
- separate capability evaluation ("can the system do this?") from regression evaluation ("did we break something that already worked?");
- escalate ambiguous judgments to multiple evaluators, panel-style review, or human inspection when stakes justify the cost.

## Cycle template

### `M0`: Manager setup

Before changing files:

- define objective and scope boundaries;
- list constraints from `AGENTS.md`, `TODO.md`, contracts, threat model, and environment;
- write the planned verification commands and measurable exit criteria;
- decide target loop depth:
  - normal major change: `2` cycles minimum,
  - security/privacy/API-contract change: `3` cycles recommended,
  - maximum `4` cycles before escalation with explicit unresolved risks.

### `C1`: First critic pass

Review the intended approach against this rubric:

- objective alignment and scope control;
- correctness and API/contract fit;
- security/privacy and abuse paths;
- testability and observability;
- operational feasibility, rollout, and rollback;
- cost, latency, and complexity overhead.

Output format:

- severity per finding: `critical`, `high`, `medium`, `low`;
- concrete failure mode;
- recommended next action.

### `B1`: First builder pass

Implement the smallest change that addresses the highest-severity findings first.

Required output:

- list each critic finding and mark it `fixed`, `partially fixed`, `deferred`, or `rejected`;
- note any new assumptions or risks introduced by the change;
- provide the exact commands that should now pass.

### `E1`: First evaluator pass

Evidence order:

1. Deterministic checks:
   - formatters,
   - linters,
   - tests,
   - builds,
   - schema validation,
   - grep/assertion checks,
   - rustdoc generation where relevant.
2. Structured heuristic checks:
   - transcript/tool-use review,
   - file-diff invariants,
   - expected artifact presence.
3. Model-based review:
   - rubric scoring,
   - natural-language assertions,
   - pairwise comparison,
   - panel/jury evaluation if a single judge is too noisy.
4. Human review:
   - only when ambiguity or impact remains high after the earlier layers.

The evaluator must distinguish:

- `pass`: sufficient evidence for this cycle,
- `fail`: blocking defect or broken invariant,
- `inconclusive`: needs another cycle or different evidence.

### `M1`: Manager decision

After evaluation:

- accept completed items into the regression baseline;
- decide whether another critic/builder/evaluator cycle is required;
- record open risks and rationale for any deferred issue;
- promote stable capability checks into regression checks when appropriate.

Repeat as `C2 -> B2 -> E2 -> M2` and, when justified, `C3 -> B3 -> E3 -> M3`.

## Pattern selection for this repo

Use the simplest pattern that fits the task:

- Single builder + evaluator loop:
  - default for routine coding, documentation, and localized bug fixes.
- Sequential workflow:
  - use when the steps are predictable and each phase feeds the next.
- Parallel critics/evaluators:
  - use when independent perspectives materially reduce risk, especially for privacy, security, or contract work.
- Manager-worker orchestration:
  - use only when the work splits cleanly across files/modules or distinct expertise areas.
- Debate / jury-style evaluation:
  - use only for ambiguous or high-stakes judgments where a single evaluator is likely to be biased or under-informed.

For `local-guard`, start with a single-agent workflow plus strong deterministic evals. Escalate to multi-agent orchestration only when independent subproblems or multi-domain coordination clearly justify the extra complexity.

## Evaluation stack for `local-guard`

Prefer this layered "Swiss cheese" evaluation stack:

1. Repo gates:
   - `cargo fmt --all`
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - `cargo test --workspace`
   - `cargo build --release --target x86_64-pc-windows-gnu`
2. Task-local evidence:
   - the verification commands listed in the active `TODO.md` milestone
   - contract/schema checks
   - artifact existence and shape checks
3. Adversarial review:
   - privacy/log-redaction regressions
   - transport/auth/session abuse paths
   - rollback and kill-switch behavior
4. Observability review:
   - telemetry coverage for the changed path
   - failure surfaces are diagnosable
5. Optional model-based judgment:
   - only when the property is hard to score mechanically
   - use a rubric and calibrate against deterministic evidence or human spot-checks

## Required artifacts per major change

For major or cross-file work, keep these artifacts aligned:

- `AGENTS.md`: mandatory operating rules and stop conditions
- `TODO.md`: execution plan, verification commands, and exit criteria
- `DEVLOG.md`: append-only chronology of critical actions and reversals
- `docs/AGENT_WORKFLOW.md`: deeper protocol and pattern-selection guidance

If the plan changes materially, update `TODO.md`. If the operating protocol changes, update `AGENTS.md` and this workflow guide together.

## Reference set

Seminal iterative-refinement references:

- Self-Refine (2023): https://arxiv.org/abs/2303.17651
- Reflexion (2023): https://arxiv.org/abs/2303.11366
- Constitutional AI (2022): https://arxiv.org/abs/2212.08073

Recent practical architecture and evaluation references:

- Anthropic, Building Effective AI Agents (published December 19, 2024): https://www.anthropic.com/research/building-effective-agents/
- Anthropic, Building Effective AI Agents: Architecture Patterns and Implementation Frameworks: https://resources.anthropic.com/building-effective-ai-agents
- Anthropic, How we built our multi-agent research system: https://www.anthropic.com/engineering/built-multi-agent-research-system
- Anthropic, Building agents with the Claude Agent SDK (verification and self-improvement loop): https://www.anthropic.com/engineering/building-agents-with-the-claude-agent-sdk/
- Anthropic, Demystifying evals for AI agents: https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents

Recent actor-critic / evaluator research references:

- LLM-ARC: Enhancing LLMs with an Automated Reasoning Critic (2024): https://arxiv.org/abs/2406.17663
- ArCHer: Training Language Model Agents via Hierarchical Multi-Turn RL (2024): https://arxiv.org/abs/2402.19446
- Natural Language Actor-Critic: Scalable Off-Policy Learning in Language Space (2025): https://arxiv.org/abs/2512.04601
- Replacing Judges with Juries (2024): https://arxiv.org/abs/2404.18796
- On scalable oversight with weak LLMs judging strong LLMs (2024): https://arxiv.org/abs/2407.04622
