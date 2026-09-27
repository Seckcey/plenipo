# Plenipo Rollout Plan

**Project:** Plenipo  
**Repository:** Seckcey/plenipo  
**Primary desktop stack:** Tauri 2 + React + TypeScript  
**Local privileged core:** Rust  
**Initial AI runtimes:** OpenAI Codex and Anthropic Claude Code  
**Primary build target:** Windows 11  
**Document purpose:** Execution plan for Claude Code / Opus 5.5 and future implementation agents.
**Plan changes:** Phase 9 is postponed and Paperclip will not be integrated; a new Sales department will be built in Plenipo later, with HubSpot as its CRM (ADR-018, 2026-09-27). Phase 10 comes next.

**Added after Phase 10 (v1.4.0):** on/off switches in Settings, including letting workers send, buy, or press Sign in without asking on allowed websites and handing CAPTCHAs to the owner (ADR-023, which amends ADR-020); and workers that learn from their work, with the owner keeping or discarding each lesson (ADR-024). Neither changes a phase.

**Added after Phase 11 (v1.6.0), at the owner's direction (2026-09-27):** a terminal panel the owner can hide, with a watch tab for each worker using a server (Phase 12); Windows servers, Server 2016 and newer, since every 8 West IT client runs them (Phase 15); and, later still and not a priority, a connection to Milepost, 8 West IT's own RMM, as another way to reach client servers (Phase 15).

---

## 1. Product Definition

Plenipo is a local-first desktop AI workforce orchestration platform.

The user gives an outcome to a persistent management hierarchy. Plenipo routes work to project coordinators and specialist worker agents, chooses an appropriate model/provider according to policy, grants only the capabilities required for the task, supervises execution, records all activity, and returns results or approval requests to the user.

Plenipo is not merely another chat client. It is the control plane for an organization of AI workers.

### Core principles

1. **Outcome-first operation**  
   The user should normally talk to a superintendent, department manager, or coordinator rather than individual model sessions.

2. **Roles are provider-independent**  
   A role such as Senior Developer, Documentation Writer, Brand Designer, or Security Reviewer is separate from the provider/model that fills it.

3. **Models are configurable policy**  
   Preferred and fallback models are selected by role and capability. Model policy belongs in settings, not hard-coded workflows.

4. **Provider subscriptions should be usable when officially supported**  
   Prefer authenticated local Codex and Claude Code runtimes where their terms and supported authentication permit it. Do not silently fall back to paid APIs.

5. **Local capabilities are explicit**  
   File access, shell execution, Git, SSH, browser control, computer use, MCP, network access, secrets, and production actions are permissions, not assumptions.

6. **Managers persist; most workers are ephemeral**  
   Superintendents, department managers, and project coordinators retain continuity. Task workers should be disposable unless persistence is explicitly required.

7. **All meaningful work is auditable**  
   Tasks, delegations, model/provider decisions, capability grants, approvals, command results, files changed, commits, pull requests, and final outcomes should be recorded.

8. **Human authority remains above the hierarchy**  
   Sensitive or destructive operations require policy-based approval. Agents do not grant themselves additional authority.

9. **Local-first, remote-capable later**  
   Desktop Plenipo owns execution. CrewOS may later provide remote visibility and approved remote control, but the browser app must not become the privileged local runtime.

10. **No phase advances on optimism**  
    Each phase has explicit acceptance criteria. Complete and verify the current phase before beginning the next.

---

## 2. Target Architecture

### 2.1 Desktop application

- Tauri 2
- React
- TypeScript
- Vite
- Rust command layer
- Windows installer
- System tray
- Local notifications
- Auto-start option
- Background local service / daemon where required

### 2.2 Core services

Plenipo should evolve into these logical components:

- **Plenipo Desktop** — user interface
- **Plenipo Core** — orchestration and domain logic
- **Plenipo Liaison** — task/message/event bus
- **Plenipo Workforce** — departments, roles, managers, coordinators, workers
- **Plenipo Router** — role/model/provider selection
- **Plenipo Runtime** — Codex, Claude Code, and future provider adapters
- **Plenipo Capabilities** — filesystem, shell, PowerShell, Git, SSH, browser, computer use, MCP
- **Plenipo Guard** — permissions, approvals, policy enforcement
- **Plenipo Ledger** — durable tasks, messages, events, executions, audit history
- **Plenipo Vault** — credential references and protected secrets
- **Plenipo Integrations** — GitHub, HubSpot (the CRM of the future Sales department), CrewOS, and future business systems

### 2.3 Initial organizational model

- Owner
  - Development Superintendent
    - Project Coordinator: Cloudline
    - Project Coordinator: Milepost
    - Project Coordinator: Waypoint
    - additional project coordinators
  - Sales Manager (postponed: a new Sales department built in Plenipo, with HubSpot as its CRM — Phase 9, ADR-018)
    - sales roles, named by the owner when the department is built
  - future departments
    - Marketing
    - Operations
    - NOC
    - Finance
    - Administration

The organization must be data-driven. Do not hard-code these departments into UI components.

---

# Phase 0 — Repository Foundation and Architectural Contract

## Goal

Create the smallest clean repository structure, architecture contract, development standards, and local build path before implementing orchestration.

## Deliverables

- Tauri 2 desktop project
- React + TypeScript frontend
- Rust Tauri backend
- repository structure
- README
- architecture document
- developer setup instructions
- environment/config conventions
- linting and formatting
- unit-test harnesses
- CI workflow
- versioning convention
- decision log / ADR directory

Suggested structure:

- apps/desktop
- crates/core
- crates/runtime
- crates/liaison
- crates/capabilities
- crates/guard
- crates/ledger
- crates/integrations
- packages/ui
- packages/types
- docs/architecture
- docs/adr
- tests

A monorepo is preferred, but keep the number of packages small until real separation is necessary.

## Technical Implementation

- Initialize Tauri 2 with React and TypeScript.
- Use Rust stable toolchain.
- Use pnpm unless there is a compelling repository-specific reason to choose another package manager.
- Configure ESLint and Prettier.
- Configure cargo fmt and clippy.
- Establish typed Tauri command boundaries.
- Define shared serializable DTOs for frontend/backend communication.
- Add a basic CI workflow for:
  - TypeScript typecheck
  - frontend tests
  - cargo fmt check
  - cargo clippy
  - cargo test
  - Tauri build smoke test where practical
- Add ADR-001 documenting the Tauri + React/TypeScript + Rust decision.
- Add ADR-002 defining local-first architecture.
- Add ADR-003 defining provider-independent roles.

## Tests

- Fresh clone can install dependencies and build.
- Tauri application launches on Windows.
- Frontend test command succeeds.
- Rust test command succeeds.
- TypeScript typecheck succeeds.
- CI succeeds on the initial branch.

## Acceptance Criteria

- A clean Windows machine with documented prerequisites can build and launch Plenipo.
- The app opens to a branded shell without runtime errors.
- No provider API keys or credentials are required merely to launch the application.
- All required checks are green.

## Dependencies

None.

## Out of Scope

- AI runtimes
- departments
- task execution
- business-system integrations (CRM, sales)
- browser control
- SSH
- production deployment
- elaborate UI polish

---

# Phase 1 — Desktop Shell and Local Runtime Supervisor

## Goal

Prove that Plenipo can safely supervise local child processes and stream their events to the desktop UI.

## Deliverables

- main Plenipo desktop shell
- navigation frame
- system tray
- local runtime supervisor
- child-process lifecycle management
- event streaming from Rust to React
- process status screen
- graceful shutdown behavior

## Technical Implementation

Create a Runtime Supervisor that can:

- start an approved executable
- pass arguments
- select a working directory
- inject explicitly allowed environment variables
- capture stdout
- capture stderr
- stream output events
- detect exit code
- cancel/terminate a process
- enforce one unique execution ID per launch
- persist basic execution metadata
- recover cleanly after UI restart

Do not expose arbitrary shell execution to the React layer. The frontend asks Core to perform an operation; the privileged Rust layer validates it.

Initial UI:

- left navigation
- company/department placeholder
- active runtimes
- task/activity panel
- settings
- logs/developer diagnostics

## Tests

- launch a harmless local test process
- receive incremental stdout
- receive stderr
- cancel a long-running test process
- detect normal and abnormal exits
- restart the UI without orphaning owned processes
- reject executable paths outside configured rules where applicable

## Acceptance Criteria

- Plenipo can launch, observe, and terminate a local child process.
- Live process activity is visible in the UI.
- Process state survives expected UI transitions.
- The frontend cannot directly execute arbitrary OS commands.

## Dependencies

Phase 0 complete.

## Out of Scope

- Codex
- Claude Code
- shell capability granted to agents
- model routing
- autonomous delegation

---

# Phase 2 — Plenipo Ledger: Durable Task and Event Model

## Goal

Create the durable system of record before multiple agents begin generating work.

## Deliverables

- SQLite database
- migrations
- repository layer
- task schema
- execution schema
- message/event schema
- artifact references
- approval records
- provider/model execution metadata
- activity timeline UI

## Technical Implementation

Use SQLite locally.

Minimum entities:

### Department
- id
- name
- description
- manager_role_id
- status

### Role
- id
- name
- description
- role_type
- persistent flag
- model_policy_id
- capability_profile_id

### Agent Instance
- id
- role_id
- runtime_provider
- provider_session_id
- project_id
- lifecycle_state
- created_at
- last_seen_at

### Project
- id
- name
- local_path
- repository_url
- department_id
- metadata

### Task
- id
- parent_task_id
- requested_by
- assigned_to
- project_id
- objective
- acceptance_criteria
- priority
- state
- created_at
- started_at
- completed_at

### Message/Event
- id
- task_id
- source
- destination
- event_type
- payload
- created_at

### Execution
- id
- task_id
- runtime
- model
- provider
- session_id
- process_id
- started_at
- ended_at
- exit_status
- usage_metadata

### Approval
- id
- task_id
- action_type
- request_payload
- state
- requested_at
- resolved_at
- resolved_by

### Artifact
- id
- task_id
- artifact_type
- local_path
- uri
- hash
- metadata

Keep schemas extensible. Do not prematurely reproduce every field of an external business system (such as a CRM).

## Tests

- migration up/down strategy
- CRUD tests
- parent/child task relations
- event ordering
- restart durability
- invalid state transition tests
- concurrent event writes
- backup/export smoke test

## Acceptance Criteria

- Kill and relaunch Plenipo while a synthetic task exists; task history remains intact.
- A task has a complete ordered activity trail.
- Invalid task transitions are rejected.
- Database corruption is not silently ignored.

## Dependencies

Phase 1 complete.

## Out of Scope

- semantic memory
- cloud database
- multi-user synchronization
- advanced analytics

---

# Phase 3 — Provider Runtime Adapters: Codex and Claude Code

## Goal

Run real OpenAI Codex and Claude Code workers under Plenipo without relying on their desktop GUIs.

## Deliverables

- runtime adapter interface
- Codex adapter
- Claude Code adapter
- provider detection
- authenticated-state detection
- session creation
- session resume
- streaming output
- cancellation
- structured results
- provider diagnostics UI

## Technical Implementation

Define a provider-neutral RuntimeAdapter contract with capabilities such as:

- detectInstallation
- detectAuthentication
- listRuntimeCapabilities
- startSession
- resumeSession
- submitTask
- streamEvents
- cancelExecution
- closeSession
- normalizeResult

### Codex

Prefer official local Codex programmatic surfaces such as app-server/SDK where supported by the installed environment. Avoid UI automation of the Codex desktop application.

### Claude Code

Prefer supported Claude Code CLI / programmatic interfaces. Use structured output modes where available. Preserve provider session IDs so Plenipo can resume the intended session.

### Authentication

- Never collect provider passwords.
- Detect supported existing authenticated states.
- Guide the user to official login/setup flows when authentication is absent.
- Do not silently convert subscription-backed execution to API-billed execution.
- Keep API fallback disabled unless explicitly configured later.

## Tests

For each runtime:

- installation detection
- unauthenticated behavior
- authenticated smoke task
- new session
- resume same session
- streamed output
- cancellation
- process crash
- rate/usage-limit failure
- malformed output
- provider unavailable

## Acceptance Criteria

From Plenipo UI:

1. launch one Codex task
2. launch one Claude Code task
3. see live activity
4. receive normalized completion result
5. resume both sessions
6. cancel an active task
7. preserve the executions in Ledger

The Codex and Claude desktop applications are not required to be open.

## Dependencies

Phases 0-2 complete.

## Out of Scope

- automatic model selection
- cross-agent delegation
- Gemini/Grok/local models
- API billing fallback
- production capabilities

---

# Phase 4 — Liaison Message Bus and Cross-Provider Handoffs

## Goal

Allow agents and managers to hand tasks to other agents through Plenipo without directly controlling one another.

## Deliverables

- Liaison service
- internal message envelope
- task handoff protocol
- context packet format
- correlation IDs
- reply routing
- parent-child task visualization
- Codex-to-Claude handoff
- Claude-to-Codex handoff

## Technical Implementation

All inter-agent work goes through Liaison.

Minimum message envelope:

- message_id
- correlation_id
- parent_task_id
- source_agent
- destination_role or destination_agent
- objective
- acceptance_criteria
- context references
- artifact references
- capability request
- priority
- timestamp

Context should be passed by reference whenever possible. Do not dump entire repositories or histories into every message.

Liaison responsibilities:

- validate sender identity
- create child task
- resolve destination
- attach minimal context
- dispatch to Runtime
- record all events
- return normalized result
- update parent task
- surface blockers

## Tests

- Codex task creates Claude child task
- Claude returns result to Codex parent
- reverse direction
- nested task depth limits
- missing destination
- failed worker
- canceled parent task
- duplicate message protection
- correlation integrity

## Acceptance Criteria

A Codex worker can request a Claude review through Liaison, Claude can complete the review, and the response appears in the originating Codex workflow with a complete Ledger trail.

The reverse path also works.

## Dependencies

Phase 3 complete.

## Out of Scope

- automatic organizational delegation
- arbitrary peer-to-peer runtime access
- cross-machine messaging

---

# Phase 5 — Workforce and Organization Engine

## Goal

Represent the company as departments, managers, coordinators, roles, projects, and ephemeral workers.

## Deliverables

- organization schema
- department definitions
- role templates
- persistent managers
- persistent project coordinators
- ephemeral workers
- org tree UI
- agent cards
- status indicators
- task ownership views

## Technical Implementation

Introduce role classes conceptually:

### Persistent
- Superintendent
- Department Manager
- Project Coordinator

### Ephemeral
- Developer
- Reviewer
- QA Engineer
- Documentation Writer
- Researcher
- Designer
- specialist workers

Managers should delegate outcomes. Workers should execute bounded tasks.

A coordinator may spawn workers through Plenipo Core but may not bypass capability policy.

Project configuration should map:

- project name
- repository
- local working directory
- department
- coordinator role
- allowed runtimes
- default capability profile

## UI

Provide:

- company overview
- departments
- managers
- project coordinators
- workers currently running
- queued work
- blocked work
- recent completed work

Do not make the org chart the only navigation method. Large organizations need searchable/list views too.

## Tests

- create department
- create role
- assign manager
- create project coordinator
- coordinator creates child worker
- worker finishes and retires
- persistent coordinator survives restart
- department/project reassignment
- orphan prevention

## Acceptance Criteria

The user can view Development as a department, select a project, give a coordinator an objective, and observe one or more workers appear under that coordinator and disappear from active workforce after completion while history remains.

## Dependencies

Phase 4 complete.

## Out of Scope

- importing an organization from another system
- model policy intelligence
- cross-device org sync

---

# Phase 6 — Model Policy and Intelligent Role Routing

## Goal

Make model/provider selection configurable by role rather than hard-coded into coordinators.

## Deliverables

- Model Registry
- Provider Registry
- Model Policy Engine
- preferred models by role
- fallback models
- capability requirements
- provider availability checks
- usage/capacity state
- routing explanation
- settings UI

## Technical Implementation

A role policy may define:

- preferred providers/models
- fallback order
- required capabilities
- disallowed providers
- subscription-only preference
- API use allowed/disabled
- minimum context capability
- vision required
- image generation required
- computer-use required
- cost preference
- cross-provider review preference

Example conceptual policies:

### Senior Developer
- preferred: Fable-class / Opus-class / Astra-class developer models as configured by the user
- fallback: user-configured alternatives

### Documentation Writer
- preferred: Sonnet-class
- fallback: Haiku-class or other configured economical model

### Brand Designer
- requires: vision + image generation
- preferred: user-selected image-capable model

Never assume a marketing name exists or a provider exposes it programmatically. Discover available models/capabilities from supported provider interfaces where possible and treat user aliases separately.

Router should return both:
- selected worker runtime/model
- reason for selection

## Tests

- preferred model available
- preferred unavailable -> fallback
- provider unauthenticated
- usage cap reached
- capability requirement mismatch
- API fallback disabled
- no eligible model
- cross-provider reviewer rule

## Acceptance Criteria

Changing a role's model preference in Settings changes the next worker Plenipo launches without modifying coordinator prompts or source code.

Plenipo clearly explains why a particular provider/model was selected.

## Dependencies

Phase 5 complete.

## Out of Scope

- autonomous purchasing
- changing subscription plans
- unsupported model scraping
- hidden provider switching

---

# Phase 7 — Capability Broker, Guard, and Human Approval

## Goal

Allow agents to use the local computer while making authority explicit, scoped, logged, and revocable.

## Deliverables

- capability registry
- capability profiles
- per-role permissions
- per-project permissions
- runtime grants
- approval queue
- deny rules
- command/event logging
- Windows credential integration
- secret-reference model

## Initial Capabilities

- filesystem.read
- filesystem.write
- shell.exec
- powershell.exec
- git.read
- git.write
- github.read
- github.write
- ssh.connect
- browser.navigate
- browser.automate
- computer.observe
- computer.control
- mcp.invoke
- network.local
- process.manage

## Security Model

Every capability request is evaluated against:

1. role policy
2. project policy
3. department policy
4. target resource
5. action risk class
6. explicit user approval rules

Sensitive examples requiring approval by default:

- production deployment
- destructive filesystem operation outside workspace
- DNS change
- credential modification
- database destructive operation
- cloud resource deletion
- financial transaction
- outbound external message where business policy requires review
- privilege escalation

Credentials must be referenced through protected storage. Do not expose raw secrets to prompts when a connector or scoped credential handle can perform the action.

## Tests

- allowed read
- denied write
- approval-required action
- approval accepted
- approval rejected
- expired approval
- path traversal attempt
- command allow/deny behavior
- secret redaction
- capability revocation during execution

## Acceptance Criteria

A Development worker can read/write only its authorized workspace and run approved development commands.

An unauthorized request is blocked and visible.

A sensitive request pauses, presents a clear approval card, and proceeds only after approval.

## Dependencies

Phase 6 complete.

## Out of Scope

- blanket unrestricted administrator access
- silent elevation
- storing plaintext secrets in SQLite
- full enterprise RBAC

---

# Phase 8 — Development Department MVP

## Goal

Reproduce the useful management behavior currently achieved through the Codex development hierarchy, but under Plenipo and across Codex + Claude.

## Deliverables

- Development Superintendent
- project coordinators
- developer worker role
- reviewer role
- QA role
- documentation role
- Git integration
- GitHub integration
- task decomposition
- review loop
- project dashboard

## Technical Implementation

User workflow:

1. User gives Development Superintendent an objective and project.
2. Superintendent resolves the project coordinator.
3. Coordinator decomposes work into bounded tasks.
4. Router assigns roles to configured models.
5. Workers operate in the authorized workspace.
6. Reviewer inspects work, preferably with cross-provider diversity when configured.
7. QA runs acceptance checks.
8. Coordinator synthesizes outcome.
9. Superintendent reports to user.
10. Sensitive actions stop for approval.

Prefer git worktrees or equivalent isolation for concurrent code workers where practical.

Ensure task agents cannot casually overwrite another worker's branch/worktree.

## Tests

Run synthetic development scenarios:

- documentation-only change
- small bug fix
- feature with implementation + review
- failed tests and repair
- concurrent workers
- reviewer requests changes
- provider failure mid-task
- coordinator restart

## Acceptance Criteria

The user can type an objective comparable to:

"Have Development implement feature X in project Y and get it ready for review."

Plenipo then delegates the work to the appropriate coordinator and mixed-provider workers without the user manually opening Codex or Claude sessions.

Final result includes:
- tasks performed
- agents/models used
- files changed
- tests executed
- commit/branch/PR information where applicable
- unresolved findings
- approvals still required

## Dependencies

Phases 0-7 complete.

## Out of Scope

- fully autonomous production releases
- every 8 West project
- sales department
- marketing department

---

# Phase 9 — Sales Department on HubSpot (postponed)

**Status: postponed** (owner direction, 2026-09-27; ADR-018). Paperclip will not be integrated: its Sales department was never working. When the owner schedules this phase, Plenipo builds a new Sales department from scratch, with the owner's existing HubSpot account as its CRM. Detail the phase in its own checklist before starting; the items below are a starting sketch.

## Goal

Give Plenipo a working Sales department, built on the same engine as Development, that uses the owner's HubSpot account as its CRM.

## Deliverables

- Sales department template: data over the Phase 5 engine, like Development (a Sales Manager and on-call sales roles; the owner names the positions)
- HubSpot connection through HubSpot's official API, with a HubSpot private app access token kept in the Vault and never shown to workers
- HubSpot tools through the capability broker and Guard (for example: search and read contacts, companies, and deals; create and update records; log notes and tasks; draft emails), with permission sets for sales roles
- source-of-truth rules
- handoff between Development and Sales work
- Sales on the Organization and Projects pages

## Architectural Rule

Do not casually duplicate mutable business state.

HubSpot remains authoritative for CRM data. Plenipo keeps its own tasks, approvals, and audit trail, and refers to HubSpot records by ID and link rather than silently creating a competing sales database.

Define explicitly which system owns:
- task state (Plenipo)
- agent execution state (Plenipo)
- contact, company, and deal records (HubSpot)
- CRM activity and notes (HubSpot)
- message drafts (decide when the phase is planned)
- approvals (Plenipo)
- audit events (Plenipo)

Capability order (Phase 10): HubSpot's API first; the browser only for screens the API does not cover.

## Tests

- load Sales department
- read contacts and deals (a HubSpot test account or a stand-in HubSpot API)
- submit approved new objective
- record changes reach HubSpot only as policy allows
- outbound message waits for approval
- HubSpot API failure (rate limit, network failure)
- expired or revoked token
- duplicate submission prevention
- cross-department handoff

## Acceptance Criteria

From one Plenipo window the user can see both Development and Sales, give work to the Sales Manager, and understand which system owns each record.

No production customer/prospect secrets are copied unnecessarily into the Plenipo Ledger. No outbound message leaves without approval.

## Dependencies

The owner schedules it (ADR-018). Development MVP stable (released as v1.0.0). Guard and the capability broker (Phase 7). A HubSpot account with a private app and the scopes the tools need. Phase 10's browser is an optional fallback, not a requirement.

## Out of Scope

- Paperclip integration, import, or migration (ADR-018)
- replacing HubSpot/CRM
- autonomous unapproved outbound sales communication
- bulk migration of CRM data into Plenipo

---

# Phase 10 — Browser Automation, Computer Use, and Advanced Local Tools

## Goal

Add controlled interaction with web applications and the graphical desktop when structured integrations are unavailable.

## Deliverables

- managed browser runtime
- browser automation capability
- optional supported browser extension
- screenshot/vision pipeline
- computer-observe capability
- computer-control capability
- domain/application policy
- user-visible session indicator
- emergency stop

## Technical Implementation

Prefer capability order:

1. official API/connector
2. CLI/SDK
3. browser automation with stable selectors
4. computer-use/visual interaction

Computer use is the fallback, not the default integration strategy.

Provide strong visual indication whenever an agent controls mouse/keyboard/browser.

Provide a global stop control accessible from the app and tray.

## Tests

- allowed site navigation
- blocked domain
- browser session launch
- screenshot capture
- form interaction in synthetic test environment
- approval-gated submit
- global stop
- timeout
- browser crash
- user takes control

## Acceptance Criteria

An authorized agent can complete a controlled browser task in a synthetic environment while every significant action appears in the activity trail and the user can immediately stop control.

## Dependencies

Guard and capability system stable. Phase 10 runs before the postponed Phase 9 (ADR-018).

## Out of Scope

- bypassing CAPTCHAs or provider security controls
- hidden browser control
- unrestricted credential harvesting
- arbitrary remote surveillance

---

# Phase 11A — Free and Pro Editions and the License Key

## Goal

Make the Free and Pro split real. Pro is a subscription: a signed key in the Vault, plus a quiet weekly check with 8 West, decides the edition. Plenipo keeps working offline for 30 days at a time, and a Free install never contacts 8 West at all. Free limits are enforced in one place, are impossible to hit silently, and lapsing never costs the owner work they have already done. Accepted in ADR-021 (Free and Pro editions under the Elastic License 2.0) and ADR-022 (subscription pricing and the weekly license check, which replaces ADR-021 decision 5). The edition table and the prices are `docs/editions.md`.

## Deliverables

- `crates/licensing/`: edition model, key verification, entitlement snapshot, check-in client
- signed license key carrying edition, holder, key id, plan, and paid-through date
- local signature verification against a public key compiled into the app, on every start
- weekly check-in to the 8 West license service, sending the key id and app version and nothing else
- 30-day offline grace, fail-open on every error
- license storage in the Vault (Windows Credential Manager)
- **Settings -> License**: enter a key, see the plan and renewal date, see when it last checked, remove the key
- single enforcement point: `Entitlements::check(limit)` -> Allowed, or Blocked with a plain-words reason
- Free limits enforced in Workforce: 1 department, 1 project, 3 workers on the job at once
- business departments (Sales on HubSpot and those after it) gated to Pro at the setup flow
- plain-words message on every blocked path, naming what Pro adds
- Ledger events for every license action and check-in result, with the key redacted
- lapse behavior that never deletes, hides, or breaks existing departments, projects, or history
- `docs/editions.md` updated with the prices and what the check-in sends

## Technical Implementation

Key format: a short signed token (Ed25519) carrying edition, holder, key id, plan, and paid-through date. The public key is compiled into the app; the signing key belongs to 8 West, stays offline, and is never in the repository. The signature is verified locally at every start, with no network.

**The weekly check.** A subscription can only work if Plenipo can learn that someone stopped paying, so Pro installs check in at most once every seven days. The request carries the key id and the app version. It never carries project or folder names, file paths, objectives, task text, worker output, model choices, or anything from the Ledger. **A Free install never checks in at all** — an owner who has not paid never contacts 8 West.

**Fail-open, always.** No internet, server down, timeout, bad response, DNS failure: Pro stays on and the check retries later. Pro drops in exactly two cases — the service explicitly reports the subscription ended, or 30 days pass with no successful check. A license service outage must never take Pro away from a paying customer, and Plenipo must stay usable on a plane.

Cancellation drops Pro at the end of the paid period, never the moment someone cancels.

Clock handling: grace is measured against the later of the system clock and the most recent time the service reported, so winding the clock back does not extend grace. A clock set forward is treated as the owner's problem to explain, not as fraud to punish.

New crate rather than a module in Core, per ADR-004 (grow crates per phase). Workforce, and each business department's setup flow, ask `licensing` for entitlements; nothing else decides for itself whether an owner is Pro.

One enforcement point, not many. Every limit resolves through `Entitlements::check(limit)`, which returns either Allowed or Blocked carrying the plain-words reason and what Pro adds. A blocked action never fails silently and never shows a raw error.

Free limits count live positions, not history. Three workers on the job means three at once; a project that has finished a hundred tasks is still within Free.

**Lapse is never destructive.** If a Pro owner drops to Free holding three departments, nothing is deleted, hidden, or stopped. Existing work stays visible, readable, and runnable to completion. Only *creating* something new past a Free limit is blocked. Destroying an owner's work over billing would be worse for Plenipo than any revenue it protected.

No hardware binding, no machine fingerprinting, no anti-tamper beyond the signature check. The Elastic License 2.0 makes working around the check a breach of licence; the code marks the boundary, the licence enforces it. Obfuscation would cost real support pain for no real protection on a source-available desktop app.

Safety is never gated. Guard, permissions, folder limits, approvals, the Vault, the control center, the Ledger, and the Activity trail are outside the entitlement system entirely, so no licensing bug can ever weaken them. All five AI tools stay in Free.

## Tests

- valid key accepted; tampered payload rejected; wrong signing key rejected; malformed key rejected
- Free: second department blocked, second project blocked, fourth simultaneous worker blocked
- Free: the whole Development flow completes on 1 department, 1 project, 3 workers
- Pro: departments, projects, and workers all unlimited
- business department setup blocked on Free, allowed on Pro
- key entered -> Pro applies without restarting the app; key removed -> Free, with nothing deleted
- lapse with three departments: everything still listed, readable, and runnable; only new creation blocked
- **no internet: Pro stays on through day 30 and drops on day 31**
- **service down, 500, timeout, garbage response: Pro stays on and the check retries**
- **service reports cancelled: Pro stays until the end of the paid period, then drops**
- **check-in body contains the key id and app version and nothing else, asserted byte for byte**
- **a Free install makes no outbound request at all**
- clock wound backwards does not extend the 30-day grace
- every blocked path returns the plain-words message, snapshot tested against the vocabulary
- Ledger records entered, accepted, rejected, removed, and each check-in result, with the key redacted
- permissions, approvals, and Guard behave identically on Free and Pro

## Acceptance Criteria

An owner with no key runs a full Development objective end to end on one department, one project, and three workers, and their Plenipo never contacts 8 West. Attempting a second project shows a plain message naming what Pro adds, and nothing fails silently. Entering a valid key unlocks Pro immediately with no restart. A Pro machine taken offline keeps Pro for 30 days. With the license service switched off entirely, Pro stays on. Removing the key returns to Free with no data lost and nothing hidden. Every license action is in the Ledger with the key redacted, and the check-in body is exactly the key id and the app version.

## Dependencies

Phase 5 (workforce engine, for where limits are counted), Phase 7 (Guard and the Vault, for where the key is stored). Accepted in ADR-021 and ADR-022. The Settings -> License screen lands here and is restyled with the rest of Settings in Phase 12A.

The 8 West license service is separate infrastructure, not built in this phase. This phase ships against a written request and response contract and a local test double, so every case above can be tested without the real service existing.

Phase 11A runs before Phase 11 and before the postponed Phase 9: the Sales department is a Pro department, so the gate exists before the department it gates.

## Out of Scope

- the 8 West license service itself, and its hosting — separate infrastructure
- payment processing, checkout, key delivery, failed-payment chasing, refunds, and sales tax — a business system outside the app
- accounts, sign-in, telemetry, analytics, crash reporting, or usage reporting of any kind
- hardware binding or machine fingerprinting
- obfuscation or anti-tamper beyond signature verification
- gating any safety, permission, approval, or record feature behind Pro
- gating any AI tool behind Pro; all four stay in Free
- the commercial licence agreement and end-user agreement text, which need an attorney

---

# Phase 11 — SSH, Remote Infrastructure, and Operations Capabilities

**Status: delivered in v1.6.0** (checklist and acceptance report in `docs/phases/phase-11-*`). Decisions: ADR-025 (servers over SSH, through Guard), with its deviations: Linux and Unix servers only, a program and its arguments rather than shell lines, **Disconnect** in place of Take over for servers, and no owner terminal or file copying yet (the terminal panel is planned in Phase 12, and Windows servers in Phase 15); and ADR-026 (SSH built into Plenipo, not Windows' ssh.exe). Servers start switched off (Settings → Switches, ADR-023), and are in the Free edition (ADR-021). Phase 11 was delivered ahead of Phase 11A, which this plan puts first: servers are Free, so there is nothing for the license to gate.

## Goal

Support Plenipo-managed work on authorized remote hosts such as development servers and infrastructure.

## Deliverables

- SSH capability
- host registry
- host fingerprints
- credential references
- command policy
- remote working directory policy
- port-forward support where justified
- audit trail
- infrastructure approval rules

## Technical Implementation

Host configuration should include:

- friendly name
- hostname
- port
- credential reference
- expected host key/fingerprint
- environment classification
- permitted roles
- permitted command classes
- approval policy

Production and development hosts must be distinguishable.

## Tests

- connect to synthetic/local SSH target
- valid host key
- changed host key
- denied role
- command execution
- output streaming
- cancellation
- connection loss
- production approval gate

## Acceptance Criteria

An authorized development or operations worker can use SSH against an explicitly configured host without receiving the raw private key in its prompt.

Unexpected host identity changes block execution.

## Dependencies

Phase 7 and stable Runtime/Ledger.

## Out of Scope

- network-wide credential discovery
- uncontrolled lateral movement
- unattended destructive production commands

---

# Phase 12A — Visual Design System (UniFi-Style Console Aesthetic)

**Status: delivered in v1.7.0** (checklist and acceptance report in `docs/phases/phase-12a-*`). Decision: ADR-029 (one design system for every screen), with the owner's choices: names under the icons on the left strip (view options later) and 13 px text. Visual regression is checked as computed-style snapshots of the Gallery in both themes, not pixel images (ADR-029 §9). The design system is in `packages/ui`, documented in `docs/design/design-system.md`.

## Goal

Establish the Plenipo visual language and shared component library before the Phase 12 screens are built, so every operator surface reads as one dense, dark, professional network-operations console rather than a set of separately styled pages.

**Reference aesthetic:** the Ubiquiti UniFi Network controller (`unifi.ui.com`) — Site Manager card grid, device list table, and port/topology detail views. Reference only for look, density, and interaction patterns; Plenipo's domain is agent workforce operations, not network monitoring.

## Design Principles

- Dark-first. Near-black application background, slightly lighter elevated surfaces, thin low-contrast dividers instead of heavy borders or drop shadows.
- Electric blue as the single accent for selection, links, and primary actions. Status color is reserved for status only.
- Information-dense. Small type, tight row heights, minimal padding. Prefer showing more rows over decorative whitespace.
- Status is always a small colored dot plus a text label, never color alone.
- Persistent left rail of icon-only section navigation, a contextual filter/facet sidebar, and a top bar carrying the current scope selector and global alerts.
- Live data is normal. Timeline strips, sparklines, and "Now" markers are first-class, not add-ons.

## Deliverables

### Design tokens
- color: background, surface, surface-raised, border, text-primary, text-secondary, text-muted, accent, and status ramp (ok / warn / error / offline / pending)
- typography scale (roughly 11–20px), tabular numerals for all metrics
- spacing, radius (small, 4–8px), elevation, and motion tokens
- light theme mapping of the same tokens; dark is the default

### Core layout shell
- icon rail with active-section indicator and tooltips
- collapsible left facet/filter panel (search box, grouped checkbox filters with counts, range sliders, "Clear Filters")
- top bar: scope selector (org / department / project), title, theme toggle, notification badge
- global banner slot for advisories and required actions, with an inline call-to-action button and dismiss

### Component library
- **Entity card** (Site Manager analogue): title, status dot and subtype line, horizontal 24h activity strip with time axis labels and a "Now" marker, a provider/owner row, and a footer row of small capability/resource icons. Used for departments, projects, and agents.
- **Card grid** with responsive column count and a card/list view toggle.
- **Dense data table**: sortable columns, status dot column, monospace/tabular numeric columns, inline links to parent entities, per-row selection checkboxes, column customization, page-size control, and a records counter.
- **Facet filter panel** bound to the table and grid.
- **Detail split view**: left properties/toggles panel, center live timeline scrubber, right topology/visual map, and a table below — the pattern from the UniFi port view, applied to an agent or task detail.
- **Topology / relationship map**: node tiles with status color fill, labeled connectors, and per-node metric captions; used for delegation trees and handoff chains.
- **Status primitives**: dot, pill, activity strip, sparkline, health bar, count badge.
- **Empty, loading (skeleton), and error states** for every component above.

### Documentation
- `docs/design/design-system.md` describing tokens, components, usage rules, and the density guidelines
- a component gallery/storybook route in the desktop app rendering every component in all states and both themes

## Technical Implementation

- Tokens defined once as CSS custom properties, generated from a single TypeScript source of truth so Rust-side or export surfaces can reuse the same values.
- Components live in a shared `packages/ui` workspace package consumed by the desktop app; no screen-level ad-hoc styling.
- No hardcoded color literals in feature code — lint rule enforces token usage.
- Virtualized rendering for tables and card grids so 1,000+ rows and 100+ cards stay responsive.
- Activity strips and timelines driven by the Phase 2 event model, with a defined downsampling strategy for long ranges.

## Tests

- visual regression snapshots of the gallery in dark and light themes
- token contrast check: all text/background pairs meet WCAG AA
- status is distinguishable without color (dot plus label present in DOM)
- keyboard navigation and focus-visible styling across rail, filters, table, and cards
- virtualized table performance with 5,000 rows
- responsive behavior at the minimum supported window size

## Acceptance Criteria

Every component in the library renders correctly in both themes with real and empty data.

Phase 12 screens can be assembled entirely from this library without introducing new one-off styles.

No feature code contains raw color values.

## Dependencies

Phase 2 event model (for activity strips and timelines). Should land before or alongside the start of Phase 12.

## Out of Scope

- copying UniFi's iconography, logo, or proprietary assets
- network-monitoring features implied by the reference screenshots
- marketing site or brand identity work beyond the application UI
- mobile layouts

---

# Phase 12 — Product UX, Notifications, Settings, and Operator Experience

## Goal

Turn the proven engine into a desktop product that the owner can understand and operate without watching raw terminal output.

All screens in this phase are assembled from the Phase 12A design system and component library. No new one-off styling.

## Deliverables

### Home / Company
- department health
- current objectives
- agents working
- blocked tasks
- approvals waiting
- recent completions

### Department View
- manager
- projects
- current workers
- queue
- performance/activity history

### Project View
- coordinator
- repository/workspace
- task tree
- running workers
- branches/PRs
- artifacts
- recent decisions

### Agent View
- role
- selected provider/model
- current task
- capabilities granted
- runtime/session
- event history

### Task View
- objective
- acceptance criteria
- delegation tree
- activity stream
- artifacts
- approvals
- final result

### Terminal panel (owner direction, 2026-09-27)

A panel at the bottom or side of the window that the owner can show, hide, and resize, like the terminal in a code editor. It amends ADR-025 (servers over SSH, through Guard), which left out a terminal for the owner; detail it in an ADR before building.

- **Your terminal:** the owner types freely, on this PC or on a server from Settings → Servers, signed in with the server's stored sign-in (never shown). The owner is in charge, so Guard does not check what the owner types; the server's pinned server ID is still checked before signing in.
- **Watch tabs:** one per worker using a server. It shows each command the worker runs and its output as it arrives, with **Stop** and **Disconnect** right there. A tab opens when a worker connects and stays readable after it disconnects.
- **Workers never type into the owner's terminal.** They keep using `ssh_run`, one command at a time through Guard, so every command is still checked, asked about when it must be, and recorded. The watch tab only shows what Guard already let through.
- Several tabs at once; the panel remembers whether it was open and its size.
- Production servers are marked red in their tabs, as everywhere else.

### Settings
- providers
- authentication state
- role/model policies
- fallback order
- capability profiles
- projects
- departments
- approval rules
- local paths
- notification preferences
- diagnostics

## Tests

- keyboard navigation
- state restoration
- large task history
- large org tree
- disconnected providers
- empty states
- error states
- accessibility smoke tests
- terminal panel: open, hide, resize, and restore after a restart
- the owner's terminal on this PC and on a synthetic SSH server, with a changed server ID refused
- a worker's watch tab shows its commands and output live, and Stop and Disconnect there end its work
- a worker cannot send keystrokes to the owner's terminal

## Acceptance Criteria

The normal user experience does not require reading terminal output, editing JSON, or memorizing session IDs.

Raw diagnostics remain available for troubleshooting.

## Dependencies

Core workflows stable.

## Out of Scope

- cosmetic redesigns that delay functionality
- mobile application
- remote multi-user console
- workers typing into the owner's terminal, or running shell lines (pipes, `&&`) through it

---

# Phase 13 — Windows Service, Installer, Updates, and Recovery

## Goal

Make Plenipo dependable as installed Windows software.

## Deliverables

- signed installer path
- uninstall
- upgrade path
- background service/daemon
- startup control
- system tray
- crash recovery
- database backup
- log rotation
- diagnostics bundle
- safe update mechanism
- version display

## Technical Implementation

Separate UI lifecycle from long-running task lifecycle.

Closing the main window must not accidentally kill authorized long-running work unless the user configured that behavior.

Define recovery states for:
- UI crash
- daemon crash
- Windows reboot
- provider process crash
- incomplete task
- interrupted database migration

## Tests

- clean install
- upgrade
- uninstall
- reboot during idle
- reboot with recoverable task metadata
- forced crash
- corrupted config
- database backup/restore
- version rollback strategy

## Acceptance Criteria

Plenipo installs and updates cleanly on a fresh Windows machine.

A UI restart does not lose the Ledger.

The user can understand and recover from a failed runtime.

## Dependencies

Core product behavior stable.

## Out of Scope

- Microsoft Store distribution unless separately approved
- macOS/Linux packaging

---

# Phase 14 — CrewOS Remote Visibility and Approved Remote Control

## Goal

Allow remote visibility and bounded command submission without moving privileged execution into the browser.

## Deliverables

- secure desktop-to-CrewOS channel
- device identity
- authenticated remote status
- task summaries
- approval notifications
- remote objective submission
- explicit local policy controlling remote actions
- offline behavior

## Architecture

CrewOS is a remote presentation/control surface.

Plenipo Desktop remains the execution authority on the local machine.

A remote request should flow:

CrewOS -> authenticated Plenipo endpoint -> Guard -> organization/router -> local runtime

Never expose an unrestricted local shell through CrewOS.

## Tests

- authenticated connection
- invalid device
- revoked session
- remote objective creation
- remote approval
- desktop offline
- replay protection
- connection loss
- local user disables remote control

## Acceptance Criteria

The user can remotely view high-level Plenipo state and submit an approved objective while local Guard policies remain authoritative.

## Dependencies

Installed desktop product stable.

## Out of Scope

- unrestricted remote desktop
- public unauthenticated endpoints
- replacing the local UI

---

# Phase 15 — Additional Providers and Department Expansion

## Goal

Prove Plenipo is genuinely provider- and department-independent.

## Deliverables

- documented provider adapter SDK/contract
- model capability discovery contract
- additional provider adapter when justified
- Marketing department templates
- Operations/NOC templates
- Windows servers for Operations (owner direction, 2026-09-27; see below)
- Milepost connection, last in this phase and not a priority (see below)
- reusable role packs
- import/export of sanitized organization configuration

## Technical Implementation

Do not add providers merely to increase a logo count.

A provider should be added when it has:
- a supported programmatic runtime
- acceptable authentication
- useful capability
- clear role fit
- stable enough execution semantics

New departments should reuse:
- Workforce
- Router
- Guard
- Liaison
- Ledger
- Capabilities

Do not fork the orchestration engine per department.

### Windows servers

Every 8 West IT client runs Windows servers, the oldest Windows Server 2016. Phase 11 supports only Linux and Unix servers (ADR-025 §11).

- **Command kinds for PowerShell:** Guard sorts PowerShell commands into the same six kinds as on Linux: look around (`Get-*`, `Test-*`), start, stop, and restart services (`Restart-Service`), install, deploy, and change files, delete, wipe, or shut down (`Remove-Item`, `Stop-Computer`), run as administrator, and other. It also keeps the never list: no reaching other computers from a server (`Enter-PSSession`, `Invoke-Command -ComputerName`, `mstsc`), no scanning, and no credential dumping.
- **Quoting and paths:** Windows quoting and `C:\` paths, with the server's folders checked the same way.
- **How Plenipo reaches them:** OpenSSH Server, built into Windows Server 2019 and newer (an optional feature that must be turned on) and a separate install on Server 2016; or PowerShell Remoting (WinRM), which is often already on inside a client's network. Choose in an ADR.
- Everything else from Phase 11 stays: the pinned server ID, sign-ins in the Vault, Test/Staging/Production, the switch, the sign, Stop all, and the Activity trail.

### Milepost connection (not a priority)

Milepost is 8 West IT's own RMM (remote monitoring and management), a separate app in its own repository. It already has an agent on every client computer that reaches out to Milepost, so no ports are opened on a client's network. A Plenipo worker could run commands on client servers through it instead of connecting directly.

- **Milepost side, built in Milepost's own repository under its own rules** (remote commands and new switches need the owner's explicit approval there): a small API only for Plenipo, with its own key that expires and can be switched off, limited to the clients the owner chooses. It lists a client's servers, runs one PowerShell command on one server, and returns the result. Milepost records the full text of every command Plenipo sends.
- **Plenipo side:** Milepost's servers appear in Settings → Servers as another way to reach a server; its key is kept in the Vault; Guard checks each command first, with the Windows command kinds above.
- **Every server reached through Milepost starts as Production,** so every command asks the owner: Milepost's agent runs commands as Windows' SYSTEM account, with full power over the server.
- Known limits of Milepost today (2026-09-27): queued commands report their output only when they finish, cannot be cancelled once running, and stop after 5 minutes; its live terminal needs Windows Server 2019 or newer.

## Tests

- provider adapter contract suite
- new department using existing engine
- model-policy switching
- mixed-provider workflow
- role pack export/import
- Windows servers: each PowerShell command kind, the never list, quoting, and folders, against a synthetic Windows SSH server
- Milepost connection (when built): a synthetic Milepost API; an expired or switched-off key is refused; every command asks the owner

## Acceptance Criteria

A new provider or department can be added without changing the fundamental task, Liaison, Guard, or Ledger architecture.

## Dependencies

Stable production architecture. Windows servers build on Phase 11. The Milepost connection needs the Milepost API, built first in Milepost's repository.

## Out of Scope

- unsupported provider hacks
- credential scraping
- provider-specific business logic in Core

---

# 3. Cross-Phase Engineering Requirements

These rules apply to every phase.

## 3.1 Security

- Least privilege by default.
- No secrets in source control.
- No secrets in prompts unless absolutely unavoidable.
- Redact sensitive values from logs.
- Validate all local IPC inputs.
- Treat model output as untrusted.
- Treat web/email/imported content as untrusted.
- Do not let agent text modify security policy.
- Capabilities are enforced in code, not merely described in prompts.

## 3.2 Reliability

- Every long-running operation has an ID.
- Every operation has timeout/cancel behavior.
- Retriable operations must be idempotent or protected against duplicates.
- Unknown external outcomes must be reconciled before retry.
- Process crashes must become explicit task events.
- No silent provider switching.

## 3.3 Observability

Record:
- task creation
- delegation
- selected role
- provider/model
- runtime session
- capabilities granted
- approvals
- process start/end
- errors
- files/artifacts
- commits/PRs
- final result

Keep raw provider logs available for diagnostics but do not make them the primary user experience.

## 3.4 Testing

Every phase must include:
- unit tests
- integration tests where applicable
- one realistic acceptance scenario
- failure-path tests
- restart/recovery tests when state is involved

Do not mark a phase complete because the happy-path demo worked once.

## 3.5 UX

The user should see organizational concepts:
- departments
- projects
- roles
- workers
- objectives
- tasks
- approvals
- results

Provider mechanics should be available when useful, but should not dominate the interface.

## 3.6 Provider Neutrality

Core business objects must not be named after specific vendors.

Good:
- RuntimeAdapter
- ModelPolicy
- AgentInstance
- Task
- Session

Avoid:
- ClaudeWorkerTask
- CodexDepartment
- AnthropicProject

Provider-specific details belong in adapters.

---

# 4. MVP Boundary

The first genuinely useful Plenipo release is complete at the end of **Phase 8**.

That MVP must allow:

1. Launch Plenipo on Windows.
2. View Development.
3. Select or name a configured software project.
4. Give the Development Superintendent an outcome.
5. Route the objective to the project's coordinator.
6. Spawn Codex and/or Claude Code workers from supported authenticated local runtimes.
7. Choose models according to role policy.
8. Allow workers to communicate only through Liaison.
9. Grant controlled filesystem/Git/shell capabilities.
10. Perform implementation, review, and test work.
11. Show live status graphically.
12. Preserve the entire task tree and audit history.
13. Pause for explicit approval on sensitive operations.
14. Return a synthesized result to the user.

Do not delay this MVP to implement Sales, CrewOS, advanced browser control, or additional providers.

---

# 5. Initial Role Templates

These are starting templates, not permanent hard-coded model choices.

## Development Superintendent

Purpose:
- accept owner objectives
- select project coordinator
- track major outcomes
- escalate blockers
- summarize completion

Persistence:
- persistent

Default capabilities:
- task management
- project lookup
- Liaison
- read-only high-level project status

## Project Coordinator

Purpose:
- decompose project objectives
- spawn workers
- coordinate dependencies
- request reviews
- judge acceptance criteria
- synthesize result

Persistence:
- persistent per project

Default capabilities:
- task management
- repository metadata
- Liaison
- worker spawn

## Senior Developer

Purpose:
- implementation
- debugging
- refactoring

Persistence:
- ephemeral

Suggested model policy:
- user-configured high-capability coding models such as Fable-class, Opus-class, or Astra-class models when available

## Code Reviewer

Purpose:
- independent review
- correctness
- maintainability
- architectural findings

Persistence:
- ephemeral

Suggested policy:
- prefer a different provider/model family from the primary implementer when configured

## QA Engineer

Purpose:
- tests
- reproduction
- acceptance verification

Persistence:
- ephemeral

## Documentation Writer

Purpose:
- README
- architecture docs
- release notes
- user/admin documentation

Persistence:
- ephemeral

Suggested policy:
- user-configured economical writing models such as Sonnet/Haiku-class models where appropriate

## Brand / Creative Designer

Purpose:
- graphics
- campaign visuals
- brand assets

Persistence:
- ephemeral

Required model capabilities:
- vision
- image generation where the task requires original imagery

---

# 6. Configuration Philosophy

Settings should eventually allow the user to express policy such as:

- Senior Developer:
  - preferred: Model A
  - second: Model B
  - fallback: Model C

- Documentation Writer:
  - preferred: Model D
  - fallback: Model E

- Brand Designer:
  - requires vision
  - requires image generation
  - preferred: best configured image-capable provider

- Security Reviewer:
  - prefer provider different from implementation provider

Global options may include:

- prefer subscription-backed runtimes
- disable paid API fallback
- use economical models for routine tasks
- reserve premium models for difficult tasks
- require cross-provider review for production-impacting code
- pause instead of switching provider when usage caps are reached
- notify when a preferred model becomes unavailable

Keep model names in user-managed configuration or discovered provider metadata wherever possible. Do not hard-code transient commercial model names throughout application logic.

---

# 7. Definition of Done for Any Task

A Plenipo implementation task is not done until:

- requested behavior is implemented
- code builds
- tests pass
- relevant failure cases are tested
- security implications are considered
- no secrets are introduced
- documentation is updated
- task artifacts are recorded
- acceptance criteria are explicitly verified

An agent saying "done" is not acceptance evidence.

---

# 8. Execution Instructions for Claude Code / Opus 5.5

When using this document as the implementation driver:

1. Read the full rollout plan before making architectural changes.
2. Determine the current completed phase from repository evidence, not assumptions.
3. Work only on the earliest incomplete phase unless explicitly instructed otherwise.
4. Create a short phase implementation checklist before coding.
5. Preserve provider-neutral abstractions.
6. Prefer small, reviewable commits.
7. Run all tests required by the phase.
8. Record deviations from this plan in an ADR when they alter architecture.
9. Do not implement later-phase functionality merely because it is interesting.
10. Do not weaken Guard, capability boundaries, or approval requirements to make a demo pass.
11. At phase completion, produce an acceptance report mapping every acceptance criterion to evidence.
12. Stop at the phase boundary for owner review if the next phase materially expands privileges or external integrations.

---

# 9. Recommended First Build Sequence

For the initial Opus 5.5 session:

1. Complete Phase 0 only.
2. Commit the repository foundation.
3. Run the complete Phase 0 test suite.
4. Produce a Phase 0 acceptance report.
5. Only after Phase 0 passes, begin Phase 1.

The early goal is not to make Plenipo look impressive.

The early goal is to make one boring, dependable vertical slice:

**Desktop UI -> Plenipo Core -> supervised local process -> streamed events -> Ledger -> UI**

Once that path is solid, Codex and Claude become runtime adapters on top of a reliable control plane rather than special cases embedded throughout the application.

---

# 10. Product North Star

The intended interaction is simple:

**Owner:** "Have Development finish feature X in Cloudline and get it ready for review."

Plenipo should be able to:

- understand which department owns the objective
- route it to the correct project coordinator
- choose the appropriate AI workers
- launch them through supported local runtimes
- give each only the authority it needs
- coordinate cross-provider collaboration
- capture work and evidence
- stop for owner decisions when required
- return one coherent result

The user manages the organization.

Plenipo manages the agents.
