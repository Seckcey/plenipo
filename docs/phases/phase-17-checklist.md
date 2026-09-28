# Phase 17 — Implementation Checklist

**Status:** design approved by the owner (2026-09-28), with the owner's addition (ADR-045);
building. To be delivered as **v1.10.0**. Built on v1.9.0 (Phase 13).

Source: `ROLLOUT_PLAN.md`, Phase 17 — The Owner's Control Over Workers (second in the order of
work, ADR-039), and the four records written for it:

- [ADR-041 (model, effort, and learning in layers)](../adr/ADR-041-model-effort-learning-layers.md)
- [ADR-042 (specialties under each role)](../adr/ADR-042-specialties.md)
- [ADR-043 (archive, bring back, and delete for good)](../adr/ADR-043-archive-bring-back-delete.md)
- [ADR-044 (prompts sized to the job)](../adr/ADR-044-prompts-sized-to-the-job.md)
- [ADR-045 (experience and the Workforce)](../adr/ADR-045-experience-and-the-workforce.md), the
  owner's addition

This checklist keeps the plan's words where it quotes the plan. The app uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md): a "layer" is a **rule** ("the
Development department's rule"), a "position" is an **agent** on the chart, a "turn" is a
**task**, and a "brief" is the worker's **full instructions** or a **short reminder**.

**Goal (plan):** "Let the owner set how every agent works — its model, effort, learning, and
specialty — at the level that fits (the organization, a department, a role, or one agent);
archive, bring back, and delete agents; and understand every option in the properties panel.
Make prompts only as long as the job needs."

## Owner decisions (2026-09-28)

- **The design is approved, and ADR-041, ADR-042, ADR-043, and ADR-044 are accepted, as
  recommended.**
- **AI companies never to use add up** across the layers ("never" means never), as ADR-041 §4
  recommends, instead of the plan's "closest wins" for that one setting.
- **Deleting a project or department for good takes along what was archived with it**, each
  leaving its short record, after offering to save the experienced agents (ADR-043 §12,
  ADR-045).
- **The owner's addition:** "When our agents start learning their jobs better, we need a way to
  score how much the agent has learned. For the agents with a lot of experience and learned
  knowledge, Plenipo needs to have a Workforce tab where we can save and store agents to. When
  deleting an agent or organization, if there are agents with higher than average score, offer
  to save them to the user's Workforce for future use. If none are selected to save and move to
  the Workforce then they get permanently deleted." Written as
  [ADR-045 (experience and the Workforce)](../adr/ADR-045-experience-and-the-workforce.md),
  **accepted as written**, with the experienced agents checked (saved) by default in the
  delete-for-good box.
- Version **1.10.0** (the owner's instruction when starting the phase).
- Numbers: **ADR-041 to ADR-044**. `main` ends at ADR-040. The one open pull request with
  records, #74 (the security fixes, Group B), numbers its three records ADR-036 to ADR-038, which
  `main` already uses; it will need new numbers when it merges, and if it takes 041–044 first,
  these are renumbered.
- Out of scope (plan): the canvas's trash can, drawer, and dragging (Phase 18); agents writing in
  an invented or hidden language; paid AI keys (Phase 16).

## Design (2026-09-28)

Written before building, from a map of the code at `b199e5d` (`main`, v1.9.0).

### 1. Model and effort in layers (ADR-041)

- **Kept in the routing setting** (`crates/router/src/config.rs`), beside today's role
  policies: `organization: ModelRule`, `departments: {id → ModelRule}`, and
  `positions: {id → ModelRule}` (the agent's own rule). A `ModelRule` has `models` (IDs in order),
  `efforts` (by model ID), `effort` (for any other model), and `neverCompanies`. `RolePolicy` gains
  `effort` too. Changes are validated in one transaction and recorded as `router.rule_changed`.
  No Ledger layout change.
- **The engine** (`engine.rs`) takes the four layers (agent, role, department, organization)
  instead of one policy: the closest layer with a model list gives the list; the effort for the
  chosen model comes from the closest layer that sets one it accepts; never-use lists add up. A
  fixed agent keeps its AI tool and model, and its effort comes from the layers. The decision
  gains `modelFrom` and `effortFrom` (the layer and its name), and the reason names them.
- **Who is in which department:** `OrgView::department_of` (the head's department, or the
  project's). `directory::decide`, the snapshot, and the conversation planner pass the position
  and its department to the Router.
- **Effort alone keeps the agent:** saving an agent's rule never touches the Ledger's position
  row, so no agent is hired. When the agent is full-time and its conversation is open, the
  conversation's recorded effort (`runtime_sessions.effort`) is updated for its next task if its
  AI tool takes that level, with an `agent.session_effort_changed` event.
- **Checks:** an effort is checked against the model's own levels
  (`RuntimeCapabilities::effort_levels_for`), with the plain refusals in ADR-041 §8.
- **Screens:** Settings → AI models gets **Model and effort rules** — the whole organization,
  each department, each role (today's table, now with "effort for any other model"), and the
  agents with their own rule — each showing what its next worker gets and why. The agent's panel
  shows its AI tool and model, its effort, and its own rule on the AI model tab.

### 2. Learning in layers (ADR-041)

- The `learning` setting gains `offRoles` and `agents: {positionId → on/off}`. Worker learning
  (Settings → Switches) stays the main switch. `learning::applies(settings, position)` decides:
  main switch off → off; else the agent's setting, else the role's, else on.
- `learning::instructions` and `learning::record` take the position, so an agent with learning
  off neither gets its role's lessons nor has lessons recorded.
- **Screens:** the agent's Job tab: learning (follow the role, on, off) and its role's (on, off,
  and Learn on its own). Events `learning.role_switched`, `learning.agent_switched`.

### 3. Specialties (ADR-042)

- **Ledger layout 9** adds `specialties` (ID, role, name, suggested title, lines and suggestions
  as JSON, built-in or yours, when made, when removed) and `positions.specialty_id`. The table is
  added to the Ledger's export list.
- **Built-in specialties** live in `crates/workforce/src/templates.rs` beside the roles, seeded
  and refreshed at every start like `ensure_roles`; yours are created, changed, and removed by
  the owner (`org.specialty_created`, `_updated`, `_removed`).
- **Instructions:** `prompt::job_text` takes the specialty and adds its lines to each of the four
  parts under "Your job as Senior Developer (Database):".
- **Hiring and changing:** `HireInput` and `PositionPatchInput` gain `specialtyId`; the Ledger
  checks it belongs to the position's role. Changing it never hires a new agent.
- **Suggestions:** shown on the AI model tab (models, with Use these) and the Work tab
  (permissions), never applied on their own.

### 4. Archive, bring back, delete for good (ADR-043)

- **Ledger layout 9** also adds `positions.deleted_at`, `departments.archived_at` and
  `deleted_at`, and `projects.archived_at` and `deleted_at`, and replaces the trigger that froze
  archived positions with one that allows only bringing back and becoming a short record (the
  "never deleted" trigger stays).
- **Archive notes** in each item's `metadata.archive`: archived with what, whether it had an
  agent, and the assignments that ended.
- **Ledger operations** (`crates/ledger/src/workforce.rs`, each one transaction):
  `archive_department`, `bring_back_position`, `bring_back_project`, `bring_back_department`,
  `delete_position_for_good`, `delete_project_for_good`, `delete_department_for_good`. Unfinished
  work is checked for every position involved and for the project's own tasks. Events:
  `org.department_archived`, `org.position_restored`, `org.project_restored`,
  `org.department_restored`, `org.position_deleted`, `org.project_deleted`,
  `org.department_deleted` (with `forGood: true`).
- **Settings that go with a deletion:** its rule in the routing setting, its learning setting,
  and a department's permission limit in Guard's setting.
- **Remove department** (the old real delete) is replaced; its command is removed.
- **Screens:** the List view gets an **Archived** tab; an archived item's panel shows Bring back
  and Delete for good; both ask first where something is lost, and Delete for good names
  everything that goes. Deleted items leave every list but still name old work.

### 5. The properties panel rebuilt

- **Tabs** (`Tabs` from `@plenipo/ui`): **Overview** (who it is, status, where it sits, its AI
  tool, model, and effort in one line, the current objective, and Give an objective — the default
  tab, so today's objective form stays where tests expect it), **Job** (role, specialty, working
  instructions, learning, lessons), **AI model** (why this model and effort, naming the rule;
  Automatic or fixed; effort; its own rule; suggestions), **Work** (running, waiting, queued,
  recent, team; live workers; its permissions and its project's permission limit; suggested
  permissions), **Team** (reports to and Move, its team, reviewer, QA, and security assignments;
  its department or project when it leads one), **Manage** (rename, hire into team, hire or let
  go, archive; for an archived item, Bring back and Delete for good).
- **A one-line "what this does"** under every option: each control has a hint, joined with
  `aria-describedby`.
- **Wider:** a `ResizeHandle` on the panel's left edge; the width (320–720 pixels) is remembered;
  the canvas keeps clear of it.
- **Test:** every option on every tab, for a full-time, an on-call, a manager, a supervisor, and
  an archived agent, has its hint; the list of options and hints is kept as a snapshot file; and
  no label or hint uses a word from the "Not" column of the word list.

### 6. Prompts sized to the job (ADR-044)

- **Measure (first):** the runtime records each step's `prompt` — `bytes`, `ownBytes`, `brief`
  (`full`, `reminder`, or `replies`), `why`, and `fullOwnBytes` — in the step's
  `usage_metadata` and its `agent.result` event. Liaison passes, with each message, the bytes it
  only passed along (the objective, context, replies). The conversation view shows it beside the
  token counts.
- **Choose:** Liaison builds both the full message and a reminder; the runtime, which knows
  whether the conversation is new, how many objectives came since the last full brief, and
  whether the AI tool shortened its memory, picks one, and the full or the one-line permissions
  note, by the rules in ADR-044 §2–§3. State is kept in memory per conversation.
- **Signals:** Claude Code's `compact_boundary` notice, ACP's `usage_update` (Kimi, Grok), and the
  Ollama helper's "left out" notice become one `MemoryShortened` event.
- **Handoffs:** the labeled request format; replies show their task ID; a record already given
  to a conversation is referred to by ID, not pasted again; "plain words, no private shorthand".
- **Tests' fake AI tool** (`plenipo-fake-agent`) learns the new format, can report a shortened
  memory, and records each prompt's size.

### 7. Experience and the Workforce (ADR-045, the owner's addition)

- **Experience** is worked out from the Ledger for each agent: 10 for each lesson it wrote that
  is still kept, 1 for each task it finished, plus what it brought from the Workforce. The
  snapshot carries each agent's score and reasons, and the organization's average (over agents
  with a finished task).
- **Ledger layout 9** adds `saved_agents` (the Workforce); saving turns the agent's position
  into a short record ("moved to your Workforce") and copies its settings, experience, and kept
  lessons. Hiring from the Workforce creates a new position with them and restores the lessons
  its role no longer has (unless you removed them).
- **Deleting for good** takes a list of agents to save first; the confirmation checks the
  experienced ones (above the average).
- **Screens:** a **Workforce** tab in the List view; Save to my Workforce on archived agents;
  Hire from my Workforce in the hire menu and the Workforce tab.

### 8. New desktop commands — the main window's alone

`set_model_rule`, `set_role_learns`, `set_agent_learning`, `create_specialty`,
`update_specialty`, `remove_specialty`, `archive_department`, `bring_back_position`,
`bring_back_project`, `bring_back_department`, `delete_position_for_good`,
`delete_project_for_good`, `delete_department_for_good`, `save_to_workforce`,
`hire_from_workforce`, `delete_saved_agent`. Each is added to `build.rs` and
`capabilities/default.json` only (not the sign window's `indicator.json`), and an IPC test calls
each from the main window and checks that another window, the sign window, and a web page are
refused. `remove_department` is removed.

### 9. Words on screen

New pairs for the word list: **rule** (for "layer", "policy layer", "precedence"); **effort for
any other model**; **specialty**; **Archived** / **Bring back** / **Delete for good** (for
"restore", "unarchive", "purge", "hard delete", "tombstone"); **short record** (for
"tombstone"); **full instructions** / **short reminder** (for "full brief", "system prompt",
"context re-injection"); **shortened its memory** (for "compaction", "context compaction"); **experience** (for "score", "XP"); **Workforce** (agents you saved to hire
again, for "talent pool", "bench", "agent library").

## Deliverables (plan)

- [ ] **Effort per agent** — with or without fixing its AI tool and model.
- [ ] **Model and effort rules in layers** — organization → department → role → agent, the
      closest wins; each layer sets an ordered list of models, the effort for each, and AI
      companies never to use.
- [ ] **Learning in layers** — organization (today's switch), each role, each agent; each role's
      "keep lessons without asking" stays.
- [ ] **Specialties under each role** — built in (Senior Developer, Designer, Security Auditor,
      Operations Engineer, Researcher, Documentation Writer) and the owner's own on any role.
- [ ] **Archive, bring back, delete for good** — for agents, departments, and projects, from an
      Archived list in the organization's List view.
- [ ] **The properties panel rebuilt** — tabs (Overview, Job, AI model, Work, Team, Manage); a
      one-line "what this does" under every option; effort and permissions shown; can be widened.
- [ ] **Prompts sized to the job** — measured and recorded per task; a short reminder on routine
      tasks; the full brief at the start, after a shortened memory, after a set number of
      objectives, and for a large job; handoffs pointing at saved records by ID, in a compact,
      labeled, plain-words format.
- [ ] **Experience and the Workforce** (the owner's addition, ADR-045) — a score for how much
      each agent has learned and done; a Workforce tab to save agents and hire them again;
      deleting for good offers to save the experienced ones.

## Technical implementation (plan)

- [ ] The Router's precedence: fixed agent → agent's own settings → role → department →
      organization → model default; the reason names the layer that decided.
- [ ] Changing a position's model keeps today's warning that a new agent is hired; changing only
      effort does not hire a new agent.
- [ ] Specialties are data, like roles; a position records role and optional specialty; lessons
      stay per role.
- [ ] Delete for good removes the item and its settings; the Ledger keeps a short record in its
      place (ID, name, role, dates, "deleted by the owner"); refused while anything has
      unfinished work; recorded as its own event.
- [ ] Prompt sizes: the byte count of Plenipo's own text on each task's record; the goal set after
      measuring — at least half off on routine tasks; no invented private language.
- [ ] Screen text follows the word list; new words go into `docs/design/vocabulary.md`.

## Tests (plan)

- [ ] Each layer sets model and effort, and the closest wins; the routing reason names the layer.
- [ ] An effort not accepted by the model is refused with a plain message.
- [ ] Changing only effort does not hire a new agent.
- [ ] Learning off at the organization stops all learning; off for one agent stops only that
      agent; on for the agent inside a role that is off follows the closest layer.
- [ ] A specialty's lines reach the worker's instructions; a position without one gets the role
      alone.
- [ ] Archive → bring back restores the agent; delete for good leaves a short record, and old
      activity still names it.
- [ ] Delete for good is refused while there is unfinished work.
- [ ] Routine tasks carry the short reminder; the first task, a shortened memory, and a large job
      carry the full brief.
- [ ] Each task's prompt size is recorded.
- [ ] The properties panel's every option has its one-line explanation (snapshot against the word
      list).
- [ ] (ADR-045) Experience counts kept lessons and finished tasks; deleting for good offers the
      agents above the average, saves the checked ones to the Workforce, and deletes the rest;
      hiring from the Workforce brings back its settings, experience, and lessons.
- [ ] End-to-end tests in the real app, with screenshots for the acceptance report.

## Owner's rules for this phase

- [ ] Plain words on screen (the word list gains the new pairs); ADRs named, not just numbered.
- [ ] No secrets asked for in chat; nothing secret committed.
- [ ] Anything touching files, programs, the network, the browser, or the screen goes through
      Guard and the capability broker (this phase adds no such path; deleting for good never
      touches the disk).
- [ ] New desktop commands are the main window's alone; the sign window and web pages are refused
      (IPC tests).
- [ ] Delete for good is refused while anything has unfinished work, keeps a short record in the
      Ledger, and asks first.
- [ ] Logs and diagnostics files never hold secrets or anything typed in the terminal (prompt
      sizes are numbers only).
- [ ] No model names in commits or pull requests.
- [ ] Version 1.10.0 everywhere, with the Phase 17 row in `docs/development/versioning.md`.
- [ ] Release notes (`docs/releases/v1.10.0.md`), the plan's Phase 17 status line and its state in
      the order of work, this checklist, and the acceptance report with screenshots in
      `evidence/phase-17/`, in Pacific time.
- [ ] A review across several areas, with a second reviewer checking each finding, before the
      final push.
- [ ] Before each push: `pnpm check`, `cargo fmt --all -- --check`,
      `cargo clippy --workspace --all-targets --locked -- -D warnings`,
      `cargo test --workspace --locked`, `pnpm bindings` with no diff (documentation-only pushes:
      `pnpm docs:check`).

## Left for the owner (on Windows)

To be listed in the acceptance report once built: the acceptance walk-through on a real PC with
real AI tools (a rule for the organization, one department, and one agent; learning off for one
agent; a Database developer; archive, bring back, archive, delete for good), and whether each AI
tool's "shortened memory" notice arrives as expected.
