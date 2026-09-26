# Plenipo

Plenipo is a local-first desktop control plane for an AI workforce. You hand an outcome to
your AI organization — VPs, managers, and supervisors that stay on the job — and Plenipo routes
the work to supervisors and specialist workers, grants only the permissions each task needs,
watches over the work, and keeps a complete record of it.

> **Status:** Phase 6 — Model policy and role routing (implemented; awaiting owner acceptance,
> so the version stays 0.6.1). **Settings → AI models** says which AI model each role's workers
> get: list your models (each AI tool's default is built in) and how hard each one thinks (effort), give each role its model choices —
> first choice, backups, what the model must be able to do, AI companies it never uses, and
> reviews by a different AI company — and see, for every role, the model its next worker would
> get and why. New positions follow their role's choices ("Auto" on the map); you can still fix
> a position to one AI tool. A usage limit never moves work to another AI company unless you
> allow it, and every worker's reason is kept in the Ledger.
>
> Phase 5 — Workforce and organization engine (accepted, v0.6.1). The
> **Organization** view is a live topology map of your AI workforce: create departments and
> projects (each comes with its manager or supervisor), drag roles from the hire palette onto a
> lead to build its team, drag positions to change who they report to or to make them a team's
> reviewer, QA evaluator, or security auditor, and give a supervisor an objective — its workers
> appear under it while they work and leave when done, with every step in the durable local
> Ledger. The chain of command reads Worker → Supervisor → Manager → VP → President (you);
> **Settings → Personalization → Titles** can rename the ranks after a U.S. military branch or
> the Mafia. Agents run on your own signed-in Claude Code and Codex (subscription sign-ins only,
> no API billing). Workers cannot change files yet (permissions arrive with Guard in Phase 7).
> See [`ROLLOUT_PLAN.md`](ROLLOUT_PLAN.md).

## Stack

| Layer            | Technology                   |
| ---------------- | ---------------------------- |
| Desktop shell    | Tauri 2                      |
| UI               | React 19 + TypeScript + Vite |
| Privileged core  | Rust (stable)                |
| Package managers | pnpm (JS), Cargo (Rust)      |
| Primary target   | Windows 11 (NSIS installer)  |

## Quick start

Prerequisites and a step-by-step Windows guide: [`docs/development/setup.md`](docs/development/setup.md).

```powershell
git clone https://github.com/Seckcey/plenipo.git
cd plenipo
corepack enable
pnpm install
pnpm dev          # run the desktop app with hot reload
```

No API keys, provider logins, or `.env` file are needed to build or launch. To run workers,
install and sign in to Claude Code and/or Codex — see the
[setup guide](docs/development/setup.md#3-ai-tools-claude-code-and-codex-phase-3-optional).

## Common commands

| Command                                                 | What it does                                                 |
| ------------------------------------------------------- | ------------------------------------------------------------ |
| `pnpm dev`                                              | Run the desktop app in development mode                      |
| `pnpm build`                                            | Build the release app and Windows installer                  |
| `pnpm check`                                            | Versions, format, lint, typecheck, frontend tests            |
| `pnpm test`                                             | Frontend unit tests (Vitest)                                 |
| `pnpm typecheck`                                        | TypeScript typecheck for all packages                        |
| `pnpm lint`                                             | ESLint                                                       |
| `pnpm format`                                           | Prettier (write)                                             |
| `pnpm bindings`                                         | Regenerate TypeScript DTOs from Rust (`packages/types`)      |
| `pnpm e2e`                                              | End-to-end tests against the release build (see setup guide) |
| `cargo test --workspace`                                | Rust unit tests                                              |
| `cargo clippy --workspace --all-targets -- -D warnings` | Rust lint                                                    |
| `cargo fmt --all`                                       | Rust format                                                  |

## Repository layout

```
apps/desktop/            React + TypeScript UI (Vite)
apps/desktop/src-tauri/  Tauri 2 Rust backend: typed command boundary, capabilities
crates/core/             Plenipo Core: provider-neutral domain types and shared DTOs
crates/ledger/           Plenipo Ledger: SQLite system of record, migrations, event trail
crates/liaison/          Plenipo Liaison: handoff protocol, context packets, replies between
                         workers
crates/runtime/          Plenipo Runtime: process supervisor, launch profiles, policy,
                         agent runtime adapters (Claude Code, Codex) and sessions
crates/workforce/        Plenipo Workforce: organization engine (positions, teams, oversight,
                         role templates), live snapshot, role routing for Liaison
crates/router/           Plenipo Router: model registry, role model policies, explained
                         choice of AI tool and model, usage limits
packages/types/          TypeScript DTOs generated from Rust (do not hand-edit)
tests/e2e/               End-to-end tests driving the real app via tauri-driver
docs/architecture/       Architecture overview
docs/adr/                Architecture Decision Records
docs/development/        Setup, configuration, versioning
docs/phases/             Phase checklists and acceptance reports
scripts/                 Repository tooling
```

Further crates from the plan (`guard`, …) are added when the
phase that needs them begins — see [ADR-004](docs/adr/ADR-004-repository-layout.md).

## Documentation

- [Architecture overview](docs/architecture/overview.md)
- [Developer setup (Windows)](docs/development/setup.md)
- [Configuration conventions](docs/development/configuration.md)
- [Versioning](docs/development/versioning.md)
- [Architecture Decision Records](docs/adr/README.md)
- [Plain words: the words the app uses](docs/design/vocabulary.md)
- [Phase checklists and acceptance reports](docs/phases/)
- [Rollout plan](ROLLOUT_PLAN.md)
