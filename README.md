# Plenipo

Plenipo is a local-first desktop control plane for an AI workforce. You hand an outcome to
a persistent management hierarchy; Plenipo routes the work to coordinators and specialist
worker agents, grants only the capabilities each task needs, supervises execution, and keeps
a complete audit trail.

> **Status:** Phase 0 — repository foundation. The app launches to a branded shell. There are
> no AI runtimes, departments, or task execution yet. See [`ROLLOUT_PLAN.md`](ROLLOUT_PLAN.md).

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

| Command                                                 | What it does                                            |
| ------------------------------------------------------- | ------------------------------------------------------- |
| `pnpm dev`                                              | Run the desktop app in development mode                 |
| `pnpm build`                                            | Build the release app and Windows installer             |
| `pnpm check`                                            | Versions, format, lint, typecheck, frontend tests       |
| `pnpm test`                                             | Frontend unit tests (Vitest)                            |
| `pnpm typecheck`                                        | TypeScript typecheck for all packages                   |
| `pnpm lint`                                             | ESLint                                                  |
| `pnpm format`                                           | Prettier (write)                                        |
| `pnpm bindings`                                         | Regenerate TypeScript DTOs from Rust (`packages/types`) |
| `cargo test --workspace`                                | Rust unit tests                                         |
| `cargo clippy --workspace --all-targets -- -D warnings` | Rust lint                                               |
| `cargo fmt --all`                                       | Rust format                                             |

## Repository layout

```
apps/desktop/            React + TypeScript UI (Vite)
apps/desktop/src-tauri/  Tauri 2 Rust backend: typed command boundary, capabilities
crates/core/             Plenipo Core: provider-neutral domain types and shared DTOs
packages/types/          TypeScript DTOs generated from crates/core (do not hand-edit)
docs/architecture/       Architecture overview
docs/adr/                Architecture Decision Records
docs/development/        Setup, configuration, versioning
docs/phases/             Phase checklists and acceptance reports
scripts/                 Repository tooling
```

Further crates from the plan (`runtime`, `liaison`, `guard`, `ledger`, …) are added when the
phase that needs them begins — see [ADR-004](docs/adr/ADR-004-repository-layout.md).

## Documentation

- [Architecture overview](docs/architecture/overview.md)
- [Developer setup (Windows)](docs/development/setup.md)
- [Configuration conventions](docs/development/configuration.md)
- [Versioning](docs/development/versioning.md)
- [Architecture Decision Records](docs/adr/README.md)
- [Rollout plan](ROLLOUT_PLAN.md)
