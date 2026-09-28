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

| ADR                                                  | Title                                                                                       | Status   |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------- | -------- |
| [001](ADR-001-desktop-stack.md)                      | Tauri 2 + React/TypeScript + Rust desktop stack                                             | Accepted |
| [002](ADR-002-local-first-architecture.md)           | Local-first architecture                                                                    | Accepted |
| [003](ADR-003-provider-independent-roles.md)         | Provider-independent roles                                                                  | Accepted |
| [004](ADR-004-repository-layout.md)                  | Minimal monorepo layout, grow crates per phase                                              | Accepted |
| [005](ADR-005-runtime-supervisor.md)                 | Runtime supervisor boundary                                                                 | Accepted |
| [006](ADR-006-ledger.md)                             | Plenipo Ledger (SQLite system of record)                                                    | Accepted |
| [007](ADR-007-runtime-adapters.md)                   | Provider runtime adapters (Codex, Claude Code)                                              | Accepted |
| [008](ADR-008-liaison.md)                            | Liaison message bus and cross-provider handoffs                                             | Accepted |
| [009](ADR-009-workforce.md)                          | Workforce organization engine and topology canvas (amended by 039)                          | Accepted |
| [010](ADR-010-plain-titles.md)                       | Plain words, chain of command, choosable ranks                                              | Accepted |
| [011](ADR-011-model-policy-routing.md)               | Router: model registry, role policies, routing                                              | Accepted |
| [012](ADR-012-brief-agent-messages.md)               | Brief messages between agents                                                               | Accepted |
| [013](ADR-013-guard-capability-broker.md)            | Guard, capability broker, and human approval                                                | Accepted |
| [014](ADR-014-adding-ai-tools.md)                    | Adding AI tools ahead of Phase 15                                                           | Accepted |
| [015](ADR-015-acp-ai-tools.md)                       | Running AI tools over ACP                                                                   | Accepted |
| [016](ADR-016-development-department.md)             | Development department: delegation, working copies, GitHub, result                          | Accepted |
| [017](ADR-017-ollama-cloud-models.md)                | Ollama cloud models through its service                                                     | Accepted |
| [018](ADR-018-sales-on-hubspot-no-paperclip.md)      | Phase 9 postponed: no Paperclip; a Sales department later, on HubSpot                       | Accepted |
| [019](ADR-019-role-working-instructions.md)          | Every role knows its job: working instructions for all roles                                | Accepted |
| [020](ADR-020-browser-and-computer-use.md)           | Plenipo's browser and computer use, through Guard (amended by 023, 028, 035)                | Accepted |
| [021](ADR-021-editions-and-license.md)               | Free and Pro editions under the Elastic License 2.0                                         | Accepted |
| [022](ADR-022-subscription-and-license-check.md)     | Subscription pricing and the weekly license check                                           | Accepted |
| [023](ADR-023-settings-switches.md)                  | On/off switches in Settings                                                                 | Accepted |
| [024](ADR-024-workers-learn-from-work.md)            | Workers learn from their work                                                               | Accepted |
| [025](ADR-025-servers-over-ssh.md)                   | Servers over SSH, through Guard                                                             | Accepted |
| [026](ADR-026-ssh-built-in.md)                       | SSH built into Plenipo (russh), not Windows' ssh.exe                                        | Accepted |
| [027](ADR-027-acp-file-access-through-plenipo.md)    | Kimi over ACP: file access through Plenipo                                                  | Accepted |
| [028](ADR-028-choosing-plenipos-browser.md)          | Choosing Plenipo's browser: Automatic, Edge, or Chrome                                      | Accepted |
| [029](ADR-029-captcha-attempts.md)                   | Workers try a CAPTCHA three times before handing it to the owner (amended by 032)           | Accepted |
| [030](ADR-030-design-system.md)                      | One design system for every screen                                                          | Accepted |
| [031](ADR-031-terminal-panel.md)                     | The terminal panel (amends 025)                                                             | Accepted |
| [032](ADR-032-captcha-checkbox-and-verdict.md)       | Workers see the CAPTCHA they try, and hear how each try went (amends 029)                   | Accepted |
| [033](ADR-033-pages-notices-settings.md)             | Home, a page for each thing, pop-up notices, and Settings in one place                      | Accepted |
| [034](ADR-034-approved-programs-run-as-the-owner.md) | Approved programs run as the owner: tickets bound to the AI tool, safer defaults            | Accepted |
| [035](ADR-035-network-gate-covers-sockets.md)        | The network gate covers beacons, sends on the page's own, and live connections (amends 020) | Accepted |
| [036](ADR-036-every-ai-model.md)                     | Every AI model worth having: paid keys with spending caps, maker and runner, routes         | Accepted |
| [039](ADR-039-owners-notes-order-of-work.md)         | The owner's notes: selling first, eight new phases, and the order of work (amends 009)      | Accepted |
