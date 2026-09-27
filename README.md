# Plenipo

Plenipo is a local-first desktop control plane for an AI workforce. You hand an outcome to
your AI organization — VPs, managers, and supervisors that stay on the job — and Plenipo routes
the work to supervisors and specialist workers, grants only the permissions each task needs,
watches over the work, and keeps a complete record of it.

> **AI tools:** Claude Code, Codex, Grok (xAI's Grok Build, run over ACP —
> [ADR-015](docs/adr/ADR-015-acp-ai-tools.md), running AI tools over ACP), Ollama's cloud models
> ([ADR-017](docs/adr/ADR-017-ollama-cloud-models.md), Ollama's cloud models through its service
> on your PC), and now Kimi (Moonshot AI's Kimi Code, over ACP with every file it reads or writes
> going through Plenipo — [ADR-021](docs/adr/ADR-021-acp-file-access-through-plenipo.md), Kimi
> over ACP, with its file reads and writes going through Plenipo). Each uses your own sign-in;
> Plenipo never uses API keys.
>
> **Status:** Phase 10 — Plenipo's browser, and the screen, mouse, and keyboard (v1.3.0). Workers can now do tasks
> on websites that have no official connection — in **Plenipo's own browser**, never yours: it
> has its own profile, so your sign-ins and saved passwords are never used. **Settings →
> Permissions → Websites** says which websites workers may open, which never, and whether others
> ask you first. Submitting a form, buying, signing in, and sending anything always wait for your
> approval, with a screenshot of the page. Workers never type passwords or secrets, and never get
> past a CAPTCHA: when a website needs you signed in, you sign in yourself. As a last resort, a
> worker you allow can see the screen and use the mouse and keyboard, and taking control asks
> you every time. Whenever a worker uses the browser or the desktop, a sign on every page says so,
> with **Take over** and **Stop all**; the Windows tray has the same Stop. Every step is in the
> Activity trail with its screenshot. Every role now also knows its job — what it does, what it
> hands back, its limits, and when to ask for help — and you can write the same for your own
> roles. Phase 9 (Sales) is postponed: a new Sales department on HubSpot comes later.
>
> Phase 8 — the Development department (accepted, v1.0.0, the first full release). Tell
> Development what you want — "implement the login page in Website and get it ready for review" —
> and it gets done without you opening Claude Code or Codex. **Projects → Set up a Development
> project** creates the Development department with its VP, the project with its Supervisor, and
> a team: a developer, a code reviewer, a QA engineer, and a documentation writer, on both AI
> tools. The VP hands your objective to the project's Supervisor, whose team works on a new
> branch in its own working copy of your project folder (your own copy is never changed):
> implement, review, fix, test, and — when you ask — open a draft pull request on GitHub, which
> waits for your approval. The **result** is Plenipo's own record: every task, who did it on
> which AI model, the files changed, the tests and whether they passed, the review and its open
> findings, the branch and pull request, and approvals still needed.
>
> Phase 7 — Permissions, Guard, and your approval (accepted, v0.8.0). Workers can now work on your computer — only inside
> their project's folder, and only as far as you allow. **Settings → Permissions** gives each
> role a permission set (read files, change files, run programs, save to git…, each Allowed,
> Ask me, or Blocked), lets a project or department narrow it, lists the programs workers may
> run without asking and the files they may never open, and keeps secrets in Windows Credential
> Manager, where workers never see them. Anything outside a worker's permissions is blocked and
> shown; sensitive actions — deploying, DNS, passwords, payments, publishing, running as
> administrator — stop for your approval with a card that says exactly what will run. The
> **Approvals** page lets you approve, deny, or revoke a worker's permissions at once, and
> everything is recorded in the Ledger.
>
> Phase 6 — Model policy and role routing (accepted, v0.7.0). **Settings → AI models** says which AI model each role's workers
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
> the Mafia. Agents run on your own signed-in Claude Code, Codex, Grok, Kimi, and Ollama (subscription sign-ins
> only, no pay-per-use API billing). See [`ROLLOUT_PLAN.md`](ROLLOUT_PLAN.md).

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
install and sign in to at least one of Claude Code, Codex, Grok, Kimi, and Ollama — see the
[setup guide](docs/development/setup.md#3-ai-tools-claude-code-codex-grok-kimi-and-ollama-optional).

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
                         agent runtime adapters (Claude Code, Codex, Grok and Kimi over ACP,
                         Ollama) and sessions
crates/workforce/        Plenipo Workforce: organization engine (positions, teams, oversight,
                         role templates), live snapshot, role routing for Liaison
crates/router/           Plenipo Router: model registry, role model policies, explained
                         choice of AI tool and model, usage limits
crates/guard/            Plenipo Guard: permission registry and sets, policy engine, folder
                         confinement, command rules, sensitive actions, secret redaction
crates/capabilities/     Capability broker: grants, Plenipo's tool server and relay, file,
                         program, git, and GitHub tools, working copies (a branch per
                         objective), approvals, Vault (OS credential store), Plenipo's
                         browser, screenshots, the screen, mouse, and keyboard, and the
                         control center (sign, Stop, Take over)
packages/types/          TypeScript DTOs generated from Rust (do not hand-edit)
tests/e2e/               End-to-end tests driving the real app via tauri-driver
docs/architecture/       Architecture overview
docs/adr/                Architecture Decision Records
docs/development/        Setup, configuration, versioning
docs/phases/             Phase checklists and acceptance reports
scripts/                 Repository tooling
```

Further crates from the plan are added when the
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
