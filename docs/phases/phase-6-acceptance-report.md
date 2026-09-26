# Phase 6 — Acceptance Report

|              |                                                                                                                                                                     |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 6 — Model Policy and Intelligent Role Routing                                                                                                                       |
| **Branch**   | `claude/phase-6` ([PR #9](https://github.com/Seckcey/plenipo/pull/9))                                                                                               |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, full `pnpm e2e`. GitHub CI: Rust, Frontend, E2E (Linux), Windows — see PR #9.                              |
| **Date**     | 2026-09-26                                                                                                                                                          |
| **Result**   | **Both Phase 6 acceptance criteria pass end to end against fake CLIs.** Owner verification with the real Claude Code and Codex CLIs on Windows is pending (§7, O2). |

Screenshots: [Settings → AI models](evidence/phase-6/models-settings.png) ·
[a role's model choice changed](evidence/phase-6/models-role-choices.png) ·
[a role's effort for a model](evidence/phase-6/models-effort.png) ·
[why the next worker gets its model](evidence/phase-6/routing-why.png) ·
[the worker on Codex](evidence/phase-6/routing-worker-codex.png) ·
[after the change: the worker on Claude Code](evidence/phase-6/routing-worker-claude.png) ·
[the reason in the Ledger's trail](evidence/phase-6/routing-trail.png) ·
[a usage limit holding work back](evidence/phase-6/models-usage-limit.png).

Test totals: **411 Rust** (Linux) · **116 frontend** · **35 end-to-end**
against the real release binary (6 Phase 1 + 6 Phase 2 + 8 Phase 3 + 5 Phase 4 + 5 Phase 5 + 5
Phase 6).

**Effort (added at the owner's review).** Each model has an optional effort level — how hard it
thinks — and each role can set its own effort for any model in its choices. Plenipo passes it to
the AI tool on every turn of the conversation (Claude Code `--effort`, Codex
`model_reasoning_effort`) and says it in the reason ("It runs at high effort (Senior Developer's
setting for it)."). This is ADR-011 §15; organization-wide presets that assign models and effort
to jobs can build on it later.

On screen, model policies are "model choices", the preferred model the "first choice", fallbacks
"backups", a position that follows its role's policy "Automatic", and providers "AI companies"
([word list](../design/vocabulary.md)). Quotes from the plan below keep the plan's words.

CI has no AI tool accounts, so every automated test drives `plenipo-fake-agent`, installed as
`claude` and `codex`. It reports the model it was given (Claude Code's persona), and
`[handoff:role:Senior Developer+usage-limit]` makes a worker report a usage limit.

## 1. Acceptance criteria → evidence

| #   | Criterion (ROLLOUT_PLAN.md)                                                                                                                   | Result (fake CLIs) | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Changing a role's model preference in Settings changes the next worker Plenipo launches without modifying coordinator prompts or source code. | **Pass**           | E2E `acceptance: a role's model choice in Settings decides its next worker, with the reason`: in **Settings → AI models** the owner makes _Codex (default model)_ the Senior Developer's first choice; the supervisor's next worker runs on Codex ([screenshot](evidence/phase-6/routing-worker-codex.png)). The owner puts _Claude Code (default model)_ first; the next worker runs on Claude Code ([screenshot](evidence/phase-6/routing-worker-claude.png)). Integration `acceptance_a_roles_model_choices_decide_its_next_worker` does the same through the Router with an owner-added model (`fake-fast`, which the worker's session reports), and checks that no position changed (`org.position_updated` absent), the supervisor kept its conversation, and its instructions — the team briefing Liaison writes — are identical before and after the change. |
| 2   | Plenipo clearly explains why a particular provider/model was selected.                                                                        | **Pass**           | Every decision carries plain sentences and a verdict per model considered. Shown for each role in Settings ([screenshot](evidence/phase-6/models-role-choices.png)), for each position in its details ("Why the next worker gets this model", [screenshot](evidence/phase-6/routing-why.png)), for each live worker, and in the Ledger's trail ("Worker brought in for Senior Developer — Claude Code (default model) is Senior Developer's first choice and is ready.", [screenshot](evidence/phase-6/routing-trail.png)); stored with the worker's task (`workforce.routing`), `org.worker_spawned`, and `org.agent_routed`. Refusals say why too ("Codex reached its usage limit, and Senior Developer waits for it…", [screenshot](evidence/phase-6/models-usage-limit.png)).                                                                                    |

## 2. Required Phase 6 tests → evidence

Unit tests in `crates/router/src/engine.rs` test the engine (pure); `crates/router/src/service.rs`
tests the Router against a real Ledger; `crates/workforce/tests/workforce.rs` runs the real
Workforce, Router, Liaison, agent runtime, supervisor, adapters, and a file-backed Ledger against
the fake CLIs.

| Test (ROLLOUT_PLAN.md)           | Result   | Evidence                                                                                                                                                                                                                                                                                                                       |
| -------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Preferred model available        | **Pass** | `preferred_model_available` (first choice chosen, rank 1, later choices "not needed"); integration and E2E acceptance tests                                                                                                                                                                                                    |
| Preferred unavailable → fallback | **Pass** | `preferred_unavailable_falls_back` (the third choice, explained by the first choice's reason); `a_usage_limit_holds_work_back_or_moves_it_on_as_the_owner_chose` ("next choice")                                                                                                                                               |
| Provider unauthenticated         | **Pass** | `provider_unauthenticated` (signed out → skipped with "is not signed in"; still being checked at startup → "is still being checked"); objectives wait for detection first                                                                                                                                                      |
| Usage cap reached                | **Pass** | `usage_cap_reached_waits_by_default_and_can_move_on`; Router `usage_limits_come_from_the_ledger_and_can_be_cleared` (reset times, a later success, "try again now"); integration `a_usage_limit_holds_work_back_or_moves_it_on_as_the_owner_chose`; E2E `a usage limit holds that AI tool back until the owner tries it again` |
| Capability requirement mismatch  | **Pass** | `capability_requirement_mismatch` (images, context size unknown or too small); integration `reviewers_come_from_another_ai_company_and_unfit_roles_are_explained` (the Designer's request is refused with the reason); E2E Settings test (the Designer shows "None right now")                                                 |
| API fallback disabled            | **Pass** | `api_fallback_disabled` (a tool signed in with an API key is skipped — "pay-per-use API billing is off" — even as first choice; with no subscription tool left, nothing is chosen); API billing shown as Off in Settings                                                                                                       |
| No eligible model                | **Pass** | `no_eligible_model` (no models; every choice ruled out, reasons listed); integration: a full-time agent refused an objective with the reason                                                                                                                                                                                   |
| Cross-provider reviewer rule     | **Pass** | `cross_provider_reviewer_rule` (prefer: another company first; prefer with none ready: the same company, said plainly; require: never the same); integration: a Code Reviewer asked by a supervisor on Claude Code runs on Codex, "a different AI company than the work it reviews (Anthropic)"                                |

Also: disallowed companies and project AI tool rules (`project_rules_and_never_used_companies`),
cost preference (`without_a_list_the_cost_preference_orders_the_registry`), registry and policy
validation (`models_are_validated`, `policies_are_validated`, built-in entries kept), removed
models leaving every list, template policies seeded once and never replacing the owner's,
usage-limit reset parsing and derivation after a restart (from the Ledger), full-time agents routed
once per conversation and kept (`a_full_time_agent_is_routed_when_its_conversation_starts_and_keeps_it`),
fixed positions unchanged by policy, Ledger rules for automatic and fixed positions
(`automatic_positions_are_routed_not_fixed`, `a_position_switches_between_fixed_and_automatic`),
atomic settings updates, IPC boundary tests for the six new commands (configuration through IPC,
input validation, refused extra fields, denial for ungranted windows and remote origins), and
frontend tests for Settings → AI models, the details panel's explanation, fixed hires, and the
trail text. All Phase 5 tests pass with positions routed by the new engine.

## 3. Defects found and fixed during Phase 6

| Found by           | Problem                                                                                                                                                                                                                           | Fix                                                                                                                                       |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| E2E (real app)     | In the Linux webview (WebKitGTK), after a model was added to a role's list, the "Add a model to the list" menu could keep showing the next model as selected; choosing it then did nothing, so a second model could not be added. | The menu is rebuilt after every change and always shows its prompt again.                                                                 |
| Integration design | A supervisor's team list named each member's AI tool, so changing a role's model choice would have changed the supervisor's instructions (criterion 1 says it must not).                                                          | Automatic members are listed without an AI tool; the acceptance test compares the supervisor's briefing before and after a policy change. |
| Clippy             | Recording the routing reason with each worker made the Ledger's handoff decision type large.                                                                                                                                      | The worker is boxed.                                                                                                                      |
| Screenshot review  | In "Your models", each row's Edit and Remove buttons sat below the row (a flex layout on a table cell).                                                                                                                           | The buttons stay in their cell.                                                                                                           |
| Screenshot review  | The explanation of a successful choice used the warning color.                                                                                                                                                                    | Warning color only when no model can take the work.                                                                                       |

## 4. Deliverables

| Deliverable (plan)            | Location                                                                                                                                                         |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Model Registry                | `crates/router/src/config.rs` (built-in default per AI tool, owner models with aliases, capabilities, context, cost); models seen in use (`Ledger::models_seen`) |
| Provider Registry             | `ToolInfo` in the Router snapshot: AI tool, company, install and sign-in, billing, usage limit, available now                                                    |
| Model Policy Engine           | `crates/router/src/engine.rs` (pure; one explanation and a verdict per model)                                                                                    |
| Preferred models by role      | `RolePolicy.models` (first choice)                                                                                                                               |
| Fallback models               | `RolePolicy.models` (backups, tried in order); no list: the registry by cost preference                                                                          |
| Capability requirements       | `RolePolicy.needs`, `minContextTokens`; the owner marks what each model can do                                                                                   |
| Provider availability checks  | Install, sign-in (subscription only), project's allowed AI tools, never-used companies                                                                           |
| Usage/capacity state          | `crates/router/src/limits.rs` (from the Ledger's turn results: reset time or an hour; lifted by a later success or "try again now")                              |
| Routing explanation           | `RouteDecision.reason` and `candidates`; recorded with workers and routed agents; shown in Settings, the map, the details panel, and the trail                   |
| Settings UI                   | `apps/desktop/src/components/models/*` (Settings → AI models), `routing/*`                                                                                       |
| Automatic and fixed positions | Ledger (`AUTOMATIC`, `route_agent`), Workforce directory and objectives, dialogs and details panel (`components/org/*`)                                          |
| Commands                      | `get_routing`, `save_model`, `remove_model`, `set_role_policy`, `set_routing_options`, `clear_usage_limit` (architecture overview §3), each granted by name      |
| Effort per model and per role | `RuntimeCapabilities.effortLevels` and each adapter's flag; `ModelInfo.effort`, `RolePolicy.efforts`, `RouteChoice.effort`; `runtime_sessions.effort` (schema 5) |
| Decision record               | [ADR-011 (how Plenipo picks each worker's AI model)](../adr/ADR-011-model-policy-routing.md)                                                                     |

## 5. Security notes

- The UI configures policy; it never picks a worker's AI tool directly. Model names pass the same
  validation as before (`[A-Za-z0-9][A-Za-z0-9._:\[\]-]{0,63}`: never a flag or a path), are sent
  as one argument by the adapters, and every command refuses unknown fields.
- API billing stays off: a tool signed in with an API key or a third-party cloud is never chosen,
  and the runtime's own refusal (ADR-007, how Plenipo runs Claude Code and Codex) remains.
- A usage limit never moves work to another AI company unless the owner chooses "next choice";
  either way the choice and its reason are recorded. Upgrading keeps every existing position on
  the AI tool it had.
- A project's allowed AI tools bind routing too: an automatic worker is routed only within them.
- No capabilities are granted (Guard, Phase 7); capability marks on models describe the model,
  not permissions.
- Agent-facing text changed only to leave automatic members' AI tools out of team lists.
- Effort levels are a fixed list per adapter and are passed as a single argument. Codex accepts
  any `model_reasoning_effort` value without complaint, so the runtime refuses a level its adapter
  does not list before anything starts, and the Router refuses to save one.

## 6. Deviations from the plan

| Deviation                                                                                                   | Why                                                                                         | Recorded    |
| ----------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ----------- |
| Configuration is one Ledger setting document, not new tables                                                | Owner configuration, validated in Rust, changed atomically with events; no migration needed | ADR-011 §2  |
| Models are the owner's list plus "seen in use", not discovered from a listing                               | The CLIs Plenipo uses do not list models; the plan forbids assuming marketing names         | ADR-011 §3  |
| "Subscription-only" and "API use allowed/disabled" are global and fixed Off                                 | API billing stays off until a later phase configures it (ADR-007)                           | ADR-011 §5  |
| Usage-limit behavior (wait or next choice) is global, not per role                                          | The plan lists it among global options (§6)                                                 | ADR-011 §8  |
| Positions are Automatic or Fixed; positions from Phase 5 stay Fixed                                         | No silent provider switching on upgrade; owners can still pin a position                    | ADR-011 §9  |
| Full-time agents are routed when their conversation starts, then keep it                                    | Conversation continuity (plan §1.6)                                                         | ADR-011 §10 |
| Built-in roles get starting policies (images for Designer, another company for reviewers, cost preferences) | The plan's example policies, without model names                                            | ADR-011 §14 |
| Effort per model and per role choice, stored with each conversation (Ledger schema 5)                       | The owner asked for it at review; the plan does not mention effort                          | ADR-011 §15 |

## 7. Owner items

| ID  | Item                                                                                                                                                              | Recommendation                   |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------- |
| O1  | ADR-011 (how Plenipo picks each worker's AI model) — **Accepted** by the owner on 2026-09-26, with effort per model and per role choice added (§15).              | Done.                            |
| O2  | Windows check with the **real** CLIs (~20 min): the steps in [phase-6-checklist.md](phase-6-checklist.md#owner-check-on-windows-20-minutes). Report anything odd. | Required for acceptance.         |
| O3  | Version stays **0.6.1** (Phase 5, accepted); Phase 6 acceptance brings 0.7.0.                                                                                     | Bump after acceptance.           |
| O4  | The Designer has no eligible model until you mark one as able to see and make images; its requests are refused and explained.                                     | Keep, or change its choices.     |
| O5  | Phase 7 (Guard: capabilities and approvals) materially expands what workers may do on this computer.                                                              | Say "start Phase 7" after O1–O2. |

## 8. Verification

| Check                                                                     | Result                                         |
| ------------------------------------------------------------------------- | ---------------------------------------------- |
| `pnpm check` (versions, format, lint, typecheck, tests)                   | Pass — 116 frontend tests                      |
| `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` | Pass                                           |
| `cargo test --workspace`                                                  | Pass — 411 tests                               |
| `pnpm e2e` against the release build (Linux, Xvfb)                        | Pass — 35 of 35, including the 5 Phase 6 tests |
| Generated TypeScript bindings                                             | Up to date (`pnpm bindings` leaves no diff)    |
| GitHub CI on the PR                                                       | Linked from the PR                             |

## 9. Phase boundary

Phase 6 is implemented and verified against fake CLIs. It is complete once the owner accepts it
(§7). Phase 7 has not been started.
