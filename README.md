# Plenipo

Plenipo is a local-first desktop control plane for an AI workforce. You hand an outcome to
a persistent management hierarchy; Plenipo routes the work to coordinators and specialist
worker agents, grants only the capabilities each task needs, supervises execution, and keeps
a complete audit trail.

> **Status:** Phase 2 — Plenipo Ledger. Tasks, events, and executions are recorded in a
> durable local SQLite ledger with a complete ordered activity trail, backups, and corruption
> detection. Plenipo can also launch, observe, and terminate approved local processes
> (built-in diagnostics only). There are no AI runtimes or departments yet. See
> [`ROLLOUT_PLAN.md`](ROLLOUT_PLAN.md).

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

No API keys, provider logins, or `.env` file are needed to build or launch.

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
crates/runtime/          Plenipo Runtime: process supervisor, launch profiles, policy
packages/types/          TypeScript DTOs generated from Rust (do not hand-edit)
tests/e2e/               End-to-end tests driving the real app via tauri-driver
docs/architecture/       Architecture overview
docs/adr/                Architecture Decision Records
docs/development/        Setup, configuration, versioning
docs/phases/             Phase checklists and acceptance reports
scripts/                 Repository tooling
```

Further crates from the plan (`liaison`, `guard`, …) are added when the
phase that needs them begins — see [ADR-004](docs/adr/ADR-004-repository-layout.md).

## Documentation

- [Architecture overview](docs/architecture/overview.md)
- [Developer setup (Windows)](docs/development/setup.md)
- [Configuration conventions](docs/development/configuration.md)
- [Versioning](docs/development/versioning.md)
- [Architecture Decision Records](docs/adr/README.md)
- [Phase checklists and acceptance reports](docs/phases/)
- [Rollout plan](ROLLOUT_PLAN.md)
