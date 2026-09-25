# Phase 0 — Implementation Checklist

Source: `ROLLOUT_PLAN.md`, Phase 0. Written before implementation per §8.4.

Current phase determined from repository evidence: the repository contained only
`ROLLOUT_PLAN.md`, so no phase is complete and Phase 0 is the earliest incomplete phase.

## Deliverables

- [ ] Monorepo skeleton (pnpm workspace + Cargo workspace), kept small
- [ ] `apps/desktop` — Tauri 2 + React + TypeScript + Vite
- [ ] `apps/desktop/src-tauri` — Rust Tauri backend with typed command boundary
- [ ] `crates/core` — provider-neutral domain crate holding shared DTOs
- [ ] `packages/types` — TypeScript DTOs generated from Rust (single source of truth)
- [ ] Branded shell UI that loads without runtime errors and needs no credentials
- [ ] README
- [ ] `docs/architecture/overview.md`
- [ ] `docs/development/setup.md` (Windows-first prerequisites)
- [ ] `docs/development/configuration.md` + `.env.example` (no secrets)
- [ ] `docs/development/versioning.md` + version consistency check
- [ ] `docs/adr/` with ADR-000 template, ADR-001, ADR-002, ADR-003 (+ ADR-004 repo layout)
- [ ] ESLint + Prettier; cargo fmt + clippy (`-D warnings`)
- [ ] Vitest frontend harness; `cargo test` Rust harness
- [ ] CI: typecheck, frontend tests, lint, format, cargo fmt, clippy, cargo test,
      binding drift check, Tauri build smoke test + launch smoke test on Windows

## Phase 0 tests (from plan)

- [ ] Fresh clone can install dependencies and build
- [ ] Tauri application launches on Windows
- [ ] Frontend test command succeeds
- [ ] Rust test command succeeds
- [ ] TypeScript typecheck succeeds
- [ ] CI succeeds on the initial branch

## Acceptance criteria (from plan)

- [ ] Clean Windows machine with documented prerequisites can build and launch Plenipo
- [ ] App opens to a branded shell without runtime errors
- [ ] No provider API keys or credentials are required to launch
- [ ] All required checks are green

## Explicitly out of scope

AI runtimes, departments, task execution, Paperclip, browser control, SSH,
production deployment, elaborate UI polish, process supervision (Phase 1).
