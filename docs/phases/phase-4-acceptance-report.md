# Phase 4 — Acceptance Report

|              |                                                                                                                                                                                           |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 4 — Liaison Message Bus and Cross-Provider Handoffs                                                                                                                                       |
| **Branch**   | `claude/phase-4` ([PR #5](https://github.com/Seckcey/plenipo/pull/5))                                                                                                                     |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, full `pnpm e2e`. GitHub CI: Rust, Frontend, E2E (Linux), Windows (tests, installer, launch smoke) — see PR #5.                   |
| **Date**     | 2026-09-26                                                                                                                                                                                |
| **Result**   | **Accepted by the owner on 2026-09-26** and released as **v0.5.0**. Both acceptance criteria pass end to end against fake CLIs in CI, and the owner confirmed the build on Windows (§10). |

Screenshots: [Codex → Claude Code review](evidence/phase-4/handoff-codex-to-claude.png) ·
[the reviewer's session](evidence/phase-4/handoff-worker-session.png) ·
[Claude Code → Codex review](evidence/phase-4/handoff-claude-to-codex.png) ·
[waiting for a reply](evidence/phase-4/handoff-waiting.png) ·
[Ledger trail and delegation tree](evidence/phase-4/handoff-ledger-trail.png).

Test totals: **294 Rust** (Linux) · **63 frontend** · **25 end-to-end** against the real release
binary (6 Phase 1 + 6 Phase 2 + 8 Phase 3 + 5 Phase 4).

CI has no provider accounts, so every automated test drives `plenipo-fake-agent`, the Phase 3
test double installed as `claude` / `codex`. For Phase 4 it also understands Liaison's
messages: a marker such as `[handoff:claude-code]` in an objective makes the fake worker end its
answer with a `plenipo-handoff` block, exactly as a real worker is instructed to.

## 1. Acceptance criteria → evidence

| #   | Criterion (ROLLOUT_PLAN.md)                                                                                                                                                          | Result (fake CLIs) | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | A Codex worker can request a Claude review through Liaison, Claude can complete the review, and the response appears in the originating Codex workflow with a complete Ledger trail. | **Pass**           | E2E `A1: a Codex worker gets a Claude Code review…` (Workers → Codex → Allow handoffs → Start task): the turn waits, a Claude Code worker reviews the Codex answer, and the Codex turn continues as step 2 with the review ([screenshot](evidence/phase-4/handoff-codex-to-claude.png)); the reviewer's own session shows the request it was given ([screenshot](evidence/phase-4/handoff-worker-session.png)). E2E `A1: the Ledger holds the complete trail…`: request, child task, blocked, reply, delivery, continuation, and result on the parent; received, dispatched, result, reply on the child; delegation tree ([screenshot](evidence/phase-4/handoff-ledger-trail.png)). Integration `codex_task_gets_a_claude_code_review_and_continues_with_it` checks the exact event order on both tasks, both messages, the same provider session resumed, and the retired worker. |
| 2   | The reverse path also works.                                                                                                                                                         | **Pass**           | E2E `A2: the reverse path — a Claude Code worker gets a Codex review` ([screenshot](evidence/phase-4/handoff-claude-to-codex.png)). Integration `claude_code_task_gets_a_codex_review_and_continues_with_it` (same assertions as criterion 1).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |

## 2. Required Phase 4 tests → evidence

Integration tests in `crates/liaison/tests/handoffs.rs` run the real Liaison, agent runtime,
supervisor, adapters, and a file-backed Ledger against the fake CLIs.

| Test (ROLLOUT_PLAN.md)                | Result   | Evidence                                                                                                                                                                                                                                                                                                 |
| ------------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Codex task creates Claude child task  | **Pass** | `codex_task_gets_a_claude_code_review_and_continues_with_it`: the child is created under the Codex task, assigned to `claude-code`, requested by `agent:codex`, in the same workflow at depth 1; E2E A1                                                                                                  |
| Claude returns result to Codex parent | **Pass** | Same test: the reply carries Claude Code's normalized result; the Codex task continues with it in the same provider session (`resume`) and succeeds; E2E A1                                                                                                                                              |
| Reverse direction                     | **Pass** | `claude_code_task_gets_a_codex_review_and_continues_with_it`; E2E A2                                                                                                                                                                                                                                     |
| Nested task depth limits              | **Pass** | `nested_handoffs_stop_at_the_depth_limit`: Codex → Claude Code → Codex → Claude Code (depths 0–3); the fourth level is refused with the reason, the deepest worker is told and finishes itself, and every reply returns to its own requester                                                             |
| Missing destination                   | **Pass** | `missing_destinations_are_refused_and_the_requester_is_told`: an unknown runtime, a role (until Phases 5–6), and another worker's session are refused, nothing is started, and the worker gets all three reasons; E2E `a missing destination is refused…`                                                |
| Failed worker                         | **Pass** | `a_failed_worker_replies_with_its_failure_and_the_requester_carries_on` (both directions): a crashed worker's reply says `crashed`; `an_unavailable_destination_fails_the_handoff_without_switching_provider`: never sent to the other provider                                                          |
| Canceled parent task                  | **Pass** | `cancelling_a_waiting_parent_cancels_its_handoffs_down_the_tree`: cancelling the waiting owner task cancels the child (itself waiting) and the running grandchild; nothing is delivered; E2E `cancelling a waiting task stops the handoff it waits for`                                                  |
| Duplicate message protection          | **Pass** | `duplicate_requests_create_one_child_and_reconciling_again_changes_nothing`: the same block twice makes one child (`liaison.duplicate_ignored`); repeated reconciliation records nothing; a second reply is not recorded                                                                                 |
| Correlation integrity                 | **Pass** | `correlation_ids_tie_each_workflow_together_and_forgeries_are_refused`: every task, message, and `liaison.*` event of a workflow carries its ID; a block claiming its own correlation ID is refused; a reply claiming another workflow is refused and recorded; the next objective starts a new workflow |

Also: restart (`a_restart_interrupts_workflows_in_flight_and_resumes_nothing`), worker-slot
queueing (`handoffs_wait_for_a_free_worker_slot`), per-answer, per-workflow, and reply-round
limits, capability requests recorded but never granted (the worker still runs with no tools),
invalid blocks explained to the worker, sessions without handoffs ignoring blocks, handoff
workers refusing owner follow-ups; Ledger unit tests for every repository operation and the
v2 → v3 migration; runtime tests for waiting, continuing, and cancelling turns; protocol and
context unit tests (fence parsing, schema, nonce delimiters); IPC boundary tests for the three
new commands and the `handoffs` flag; frontend tests for the handoff UI, the delegation tree,
and the `liaison.*` trail descriptions.

## 3. Defects found and fixed during Phase 4

| Found by                           | Problem                                                                                                                                                                                                                                                                  | Fix                                                                                                                                                                                                                             |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Integration test                   | A reply claiming another workflow, sent to a request that was already answered, came back as "already answered" instead of being refused and recorded.                                                                                                                   | The Ledger checks a reply's claims (request, correlation ID, child task) before anything else, and a reply must come from the request's own child.                                                                              |
| Code review                        | While the owner cancelled a waiting turn, a reply delivery arriving at the same moment was told "not waiting", which would have failed a task that was being cancelled.                                                                                                  | Continuing a turn that is being cancelled answers "busy"; Liaison retries, then sees the turn cancelled and discards the replies.                                                                                               |
| Integration test                   | Only a session's first objective carried Liaison's instructions; a later objective in the same session could not hand off reliably.                                                                                                                                      | Every owner objective in a handoff session restates the instructions and the current destinations.                                                                                                                              |
| Integration test                   | Two Liaison events (a refused sender, a failed delivery) did not carry the workflow's correlation ID.                                                                                                                                                                    | Both carry it; the correlation test checks every `liaison.*` event.                                                                                                                                                             |
| Frontend review                    | A session snapshot taken while a turn waited could arrive after the turn had continued and show it waiting again.                                                                                                                                                        | The store orders a turn's progress (run, wait, run again, finish) and never moves it backwards; the session's running/waiting markers follow its turns. Tests.                                                                  |
| E2E                                | The delegation tree showed a runtime ID for the root task but labels for the others.                                                                                                                                                                                     | Core supplies each node's runtime label.                                                                                                                                                                                        |
| E2E (intermittent, release CI)     | All views share one scroll area, and switching views kept the previous view's scroll position, so a view could open part-way down. After the Workers view was scrolled, an end-to-end click in Activity missed its target in about 1 run in 3.                           | Each view now opens at its top (unit test); the test scrolls with a DOM call instead of a WebDriver wheel action.                                                                                                               |
| E2E (intermittent, release checks) | Between recording a turn's wait (or its end) and releasing its step, the runtime reported the turn as still running. A Workers snapshot read in that instant showed a waiting turn as "Working…" until the turn next changed; the cancel test timed out in 1 of 10 runs. | The runtime reports the recorded state as soon as a step's result is recorded, and the Workers store never lets a snapshot bring "running" back. A runtime test reads the session inside that instant; a store test replays it. |

## 4. Deliverables

| Deliverable (plan)                 | Location                                                                                                                                |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| Liaison service                    | `crates/liaison/src/service.rs` (`Liaison`: turn hook, validation, dispatch, reconciliation, queries)                                   |
| Internal message envelope          | `liaison_messages` (`crates/ledger/migrations/0003_*`), `crates/ledger/src/liaison.rs` (atomic records, dedupe keys, state guards)      |
| Task handoff protocol              | `crates/liaison/src/protocol.rs` (`plenipo-liaison/1`: fenced blocks, strict schema, limits), worker instructions in `context.rs`       |
| Context packet format              | `crates/liaison/src/context.rs` (`plenipo-context/1`: references resolved and capped, nonce-delimited)                                  |
| Correlation IDs                    | Task metadata `liaison.correlationId`/`depth`; every message and `liaison.*` event                                                      |
| Reply routing                      | Replies recorded when a child ends; delivered as the requester's next step in the same provider session (`AgentRuntime::continue_turn`) |
| Parent-child task visualization    | Workers: steps, handoff cards, worker sessions (`views/WorkersView.tsx`, `components/Handoffs.tsx`); Activity: delegation tree          |
| Codex-to-Claude / Claude-to-Codex  | Both directions, tested end to end                                                                                                      |
| Runtime extension points (neutral) | `crates/runtime/src/agent/service.rs`: multi-step and waiting turns, turn-end hook, preassigned sessions, opaque metadata, "busy" error |
| Commands                           | `get_task_handoffs`, `get_task_tree`, `get_liaison_overview`; `start_agent_session` takes `handoffs`                                    |
| Test double                        | `plenipo-fake-agent` handoff behaviors (`crates/runtime/src/bin/plenipo-fake-agent.rs`)                                                 |
| Decision record                    | [ADR-008](../adr/ADR-008-liaison.md)                                                                                                    |

## 5. Security notes

- Workers never control each other: a request is text in a worker's answer, parsed as
  untrusted input with a strict schema. Identity fields are refused; the sender is whoever
  Plenipo's own records say ran that turn. Workers can address runtimes only — never another
  worker's session — and a request never falls back to another provider.
- The UI can only allow handoffs for a new session, read handoffs, and cancel a turn. It cannot
  create a handoff, name an address, or grant anything; the new commands are read-only and
  validated (IPC tests).
- A child receives only what the requester referenced, resolved by Liaison: its own answer,
  short excerpts, and tasks or artifacts of the same workflow (artifacts by reference, never
  contents), capped and delimited with a nonce the requester cannot know.
- No capabilities are granted: requests are recorded and denied until Guard (Phase 7).
  Handoff workers get the same least-privilege posture as any worker (Claude Code: no tools;
  Codex: read-only sandbox) in their own empty workspace.
- Replies are another agent's text. They are delimited and described as data to evaluate, not
  instructions; with no tools, their effect is limited to text.
- Handoffs use additional turns on the owner's subscriptions; the limits (depth 3, 3 requests
  per answer, 5 reply rounds, 12 handoffs per workflow) bound them, and handoffs are off unless
  the owner allows them for a task.

## 6. Deviations from the plan

| Deviation                                                                                       | Why                                                                                                 | Recorded    |
| ----------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ----------- |
| Workers request handoffs with a structured block in their answer, not a tool call or MCP server | A tool would grant agents a capability before Guard can govern it                                   | ADR-008 §1  |
| Destinations are runtimes; `role:` destinations are refused as missing until Phases 5–6         | Role routing needs the Workforce engine and model policy                                            | ADR-008 §2  |
| The requesting task waits (`blocked`) and continues as a new step of the same task              | A finished task cannot truthfully own children still working for it                                 | ADR-008 §6  |
| The Ledger-backed runtime stores moved from the desktop app into `crates/liaison`               | Liaison records handoffs in the same transactions as turn state, and its tests need the real stores | ADR-008 §13 |
| Capability requests are always denied                                                           | Phase 4 grants no capabilities (Guard, Phase 7)                                                     | ADR-008 §10 |

## 7. Owner items

| ID  | Item                                                                                                                                                                                             | Recommendation                                      |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------- |
| O1  | ADR-008 is **Proposed**.                                                                                                                                                                         | Accept or amend.                                    |
| O2  | Windows check with the **real** CLIs and your subscription sign-ins (~15 min): the steps in [phase-4-checklist.md](phase-4-checklist.md#owner-check-on-windows-15-minutes). Report anything odd. | Required for acceptance (both criteria, real CLIs). |
| O3  | Version stays **0.4.0** until O2 passes; then **0.5.0** per the phase convention.                                                                                                                | Bump after O2.                                      |
| O4  | Phase 5 (Workforce engine: departments, roles, coordinators, workers) builds on Liaison for delegation.                                                                                          | Say "start Phase 5" after O1–O2.                    |

Things only the real CLIs can confirm (O2): that each CLI follows Liaison's written
instructions and produces a valid `plenipo-handoff` block when asked (an invalid block is
refused and the worker is told why, so the task still finishes), and how long a round trip
takes on real subscriptions.

## 8. Verification

| Check                                                                     | Result                                                                                                                                        |
| ------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `pnpm check` (versions, format, lint, typecheck, tests)                   | Pass — 63 frontend tests                                                                                                                      |
| `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` | Pass                                                                                                                                          |
| `cargo test --workspace`                                                  | Pass — 294 tests, including 16 Liaison integration tests (repeated 10 times in a row without a failure)                                       |
| `pnpm e2e` against the release build (Linux, Xvfb)                        | Pass — 25 of 25, including the 5 Phase 4 tests                                                                                                |
| Generated TypeScript bindings                                             | Up to date (`pnpm bindings` leaves no diff)                                                                                                   |
| GitHub CI on PR #5                                                        | Rust, Frontend, E2E (Linux), and Windows (tests, installer, launch smoke) — green on every pushed commit so far; final run linked from the PR |

## 9. Phase boundary

Phase 4 is complete. Phase 5 (Workforce and organization engine) has not been started (§7, O4).

## 10. Owner sign-off (2026-09-26)

| Item                            | Outcome                                                                                                |
| ------------------------------- | ------------------------------------------------------------------------------------------------------ |
| O1 ADR-008                      | **Accepted** by owner                                                                                  |
| O2 Windows check with the build | **Passed** — reported by owner ("everything looks and works great")                                    |
| O3 Version                      | Bumped to **0.5.0**; released as `v0.5.0` (tag on the release merge commit) with the Windows installer |
| O4 Phase 5                      | Awaiting the owner's go-ahead                                                                          |

Phase 4 is **accepted**.
