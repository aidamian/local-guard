# AGENTS.md

Guidance for coding agents and contributors working in this repository.

## File Roles

- `README.md` holds repository/project description and onboarding context.
- `TODO.md` is the single source of truth for product brief and execution plan.
- `AGENTS.md` contains horizontal instructions for contributors and coding agents.

## Planning protocol

- Keep `TODO.md` as the single source of project execution state.
- Use a `Critic -> Builder -> Critic -> Builder` refinement loop when creating or revising major plans.
- Every milestone in `TODO.md` must have explicit verification commands and exit criteria.

## Documentation hygiene

- Keep roles clean across docs (no product brief duplication in `README.md` or `AGENTS.md`).
- Update `Last updated` in `TODO.md` when plan or product brief content changes.
- Ensure verification commands in `TODO.md` stay runnable and current.
