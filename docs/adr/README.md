# Architecture Decision Records

An ADR records one architecturally significant decision: context, the decision, and its
consequences. Per the rollout plan (§8.8), any deviation from `ROLLOUT_PLAN.md` that alters
architecture must be recorded here.

## Process

1. Copy [`ADR-000-template.md`](ADR-000-template.md) to `ADR-NNN-short-title.md` (next number).
2. Status starts as **Proposed**; the owner marks it **Accepted**.
3. Never rewrite an accepted ADR's decision. Supersede it with a new ADR and set the old one to
   **Superseded by ADR-NNN**.

## Index

| ADR                                          | Title                                             | Status   |
| -------------------------------------------- | ------------------------------------------------- | -------- |
| [001](ADR-001-desktop-stack.md)              | Tauri 2 + React/TypeScript + Rust desktop stack   | Accepted |
| [002](ADR-002-local-first-architecture.md)   | Local-first architecture                          | Accepted |
| [003](ADR-003-provider-independent-roles.md) | Provider-independent roles                        | Accepted |
| [004](ADR-004-repository-layout.md)          | Minimal monorepo layout, grow crates per phase    | Accepted |
| [005](ADR-005-runtime-supervisor.md)         | Runtime supervisor boundary                       | Accepted |
| [006](ADR-006-ledger.md)                     | Plenipo Ledger (SQLite system of record)          | Accepted |
| [007](ADR-007-runtime-adapters.md)           | Provider runtime adapters (Codex, Claude Code)    | Accepted |
| [008](ADR-008-liaison.md)                    | Liaison message bus and cross-provider handoffs   | Accepted |
| [009](ADR-009-workforce.md)                  | Workforce organization engine and topology canvas | Accepted |
| [010](ADR-010-plain-titles.md)               | Plain words, chain of command, choosable ranks    | Accepted |
| [011](ADR-011-model-policy-routing.md)       | Router: model registry, role policies, routing    | Accepted |
| [012](ADR-012-brief-agent-messages.md)       | Brief messages between agents                     | Proposed |
