# Phase 0 — Implementation Checklist

**Status:** complete — see [phase-0-acceptance-report.md](phase-0-acceptance-report.md).

Source: `ROLLOUT_PLAN.md`, Phase 0. Written before implementation per §8.4.

Current phase determined from repository evidence: the repository contained only
`ROLLOUT_PLAN.md`, so no phase is complete and Phase 0 is the earliest incomplete phase.

## Deliverables

- [x] Monorepo skeleton (pnpm workspace + Cargo workspace), kept small
- [x] `apps/desktop` — Tauri 2 + React + TypeScript + Vite
- [x] `apps/desktop/src-tauri` — Rust Tauri backend with typed command boundary
- [x] `crates/core` — provider-neutral domain crate holding shared DTOs
- [x] `packages/types` — TypeScript DTOs generated from Rust (single source of truth)
- [x] Branded shell UI that loads without runtime errors and needs no credentials
- [x] README
- [x] `docs/architecture/overview.md`
- [x] `docs/development/setup.md` (Windows-first prerequisites)
- [x] `docs/development/configuration.md` + `.env.example` (no secrets)
- [x] `docs/development/versioning.md` + version consistency check
- [x] `docs/adr/` with ADR-000 template, ADR-001, ADR-002, ADR-003 (+ ADR-004 repo layout)
- [x] ESLint + Prettier; cargo fmt + clippy (`-D warnings`)
- [x] Vitest frontend harness; `cargo test` Rust harness
- [x] CI: typecheck, frontend tests, lint, format, cargo fmt, clippy, cargo test,
      binding drift check, Tauri build smoke test + launch smoke test on Windows

## Phase 0 tests (from plan)

- [x] Fresh clone can install dependencies and build
- [x] Tauri application launches on Windows
- [x] Frontend test command succeeds
- [x] Rust test command succeeds
- [x] TypeScript typecheck succeeds
- [x] CI succeeds on the initial branch

## Acceptance criteria (from plan)

- [x] Clean Windows machine with documented prerequisites can build and launch Plenipo
- [x] App opens to a branded shell without runtime errors
- [x] No provider API keys or credentials are required to launch
- [x] All required checks are green

## Explicitly out of scope

AI runtimes, departments, task execution, Paperclip, browser control, SSH,
production deployment, elaborate UI polish, process supervision (Phase 1).
