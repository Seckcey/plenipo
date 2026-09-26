# Phase 4 — Implementation Checklist

**Status:** accepted by the owner on 2026-09-26; released as v0.5.0 (see the
[acceptance report](phase-4-acceptance-report.md)).

Source: `ROLLOUT_PLAN.md`, Phase 4 — Liaison Message Bus and Cross-Provider Handoffs.
Phase 3 accepted (v0.4.0, owner sign-off 2026-09-26). Owner approved starting Phase 4 (Phase 3
report §7, O5).

**Goal:** allow agents and managers to hand tasks to other agents through Plenipo without
directly controlling one another.

## Design decisions (details in ADR-008)

- **Agents ask; Liaison decides.** Workers still have no tools (Phase 3 posture until Guard,
  Phase 7), so a worker requests a handoff by ending its answer with a fenced
  `plenipo-handoff` JSON block. Liaison parses it as untrusted input with a strict schema:
  unknown fields, identity fields (`source`, `messageId`, `correlationId`, …), oversized
  context, and unknown capabilities are rejected. Handoffs are opt-in per session ("Allow
  handoffs"), so ordinary sessions never act on such text.
- **Sender identity comes from Plenipo, never from agent text.** The source of a request is
  the session and task that Plenipo itself ran; Liaison checks that the task is running in
  that session and that the session allows handoffs.
- **Destinations:** a runtime (`claude-code`, `codex`, or `runtime:<id>`) means "a new
  ephemeral worker session on that runtime". Existing sessions cannot be addressed (no
  peer-to-peer access). Role destinations (`role:<name>`) are part of the envelope format
  but resolve only once the Workforce engine and model policy exist (Phases 5–6); until then
  they are rejected as a missing destination. No fallback to another provider, ever.
- **Envelope** (persisted in the Ledger): message ID, correlation ID, parent task ID, source
  agent, destination role or agent, objective, acceptance criteria, context references,
  artifact references, capability request, priority, timestamp; replies add `inReplyTo` and
  the child's normalized result.
- **Context by reference.** A request may reference the requester's own answer, tasks and
  artifacts of the same workflow, and short inline excerpts; Liaison resolves them into a
  capped context packet (`plenipo-context/1`) for the child. Nothing else is attached.
- **Correlation:** every workflow (an owner task and everything delegated from it) has one
  correlation ID, carried by every message, child task, and Liaison event. A reply must match
  its request's message ID, correlation ID, and child task (its destination is always the
  requester), and a request is answered at most once.
- **A task waits for its handoffs, then continues.** The requesting task moves to `blocked`
  ("waiting for handoff replies") in the same transaction that records the requests and
  creates the child tasks. When every reply is in, Liaison resumes the same provider session
  with the replies as a new _step_ of the same task, which then finishes normally (or hands
  off again). The session takes no other work while it waits.
- **Reply routing by reconciliation.** Liaison reacts to committed Ledger facts and
  reconciles: answer finished children, dispatch accepted requests when a worker slot is
  free, deliver complete reply sets, cascade cancellation from cancelled parents, discard
  replies to tasks that stopped waiting, and retire finished handoff workers. Every action is
  idempotent and guarded by the recorded state.
- **Limits:** depth 3, at most 3 requests per answer, 5 reply rounds per task, 12 handoffs per
  workflow, the global cap of 4 running turns (handoffs queue for a free slot), the 30-minute
  turn limit.
- **Capabilities:** requested capabilities are recorded and denied — none can be granted
  before Guard (Phase 7). Children run with the Phase 3 least-privilege posture in their own
  empty workspace.
- **Restart:** workflows in flight when Plenipo stopped are recorded as interrupted; nothing is
  dispatched or resumed automatically at startup.

## Deliverables

- [x] Liaison service (`crates/liaison`): turn hook, validation, dispatch, reconciliation
- [x] Internal message envelope (requests and replies), persisted with dedupe keys
- [x] Task handoff protocol (`plenipo-liaison/1`): directive format, worker instructions
- [x] Context packet format (`plenipo-context/1`)
- [x] Correlation IDs on messages, child tasks, and every Liaison event
- [x] Reply routing back to the requesting task (continuation step in the same session)
- [x] Parent-child task visualization (Workers handoff cards, Activity delegation tree)
- [x] Codex-to-Claude handoff
- [x] Claude-to-Codex handoff
- [x] Ledger migration 0003 (`liaison_messages`), up/down and v2 → v3 tested
- [x] Runtime: multi-step turns, suspend/continue, cancel while waiting, busy vs not ready
- [x] Fake CLI handoff behaviors for tests (`plenipo-fake-agent`)
- [x] ADR-008; architecture, configuration, setup, README updated

## Phase 4 tests (from plan)

- [x] Codex task creates Claude child task
- [x] Claude returns result to Codex parent
- [x] Reverse direction
- [x] Nested task depth limits
- [x] Missing destination
- [x] Failed worker
- [x] Canceled parent task
- [x] Duplicate message protection
- [x] Correlation integrity

Also: restart recovery, worker-slot queueing, per-answer and per-workflow limits, capability
requests denied, forged identity fields rejected, non-handoff sessions ignore directives, IPC
boundary tests for new commands, end to end through the real app.

## Acceptance criteria (from plan)

- [x] A Codex worker can request a Claude review through Liaison, Claude can complete the
      review, and the response appears in the originating Codex workflow with a complete
      Ledger trail.
- [x] The reverse path also works.

CI proves both against fake CLIs (ticked above); the owner verifies both with the real CLIs on
Windows (below) before acceptance.

### Owner check on Windows (~15 minutes)

1. Both CLIs installed and signed in (setup guide §3): **Runtimes** → **Re-check** shows both
   **Ready**.
2. **Workers** → Codex → tick **Allow handoffs to other workers** → objective: _"Write a Python
   function that checks whether a string is a valid ISO 8601 date. Before you finish, ask
   claude-code to review it for bugs, then give me the final version."_ → **Start task**.
   Expected: the turn shows **Waiting for replies** and a handoff card "→ Claude Code …
   Worker running"; then the card turns **Answered**, the turn runs **Step 2 · continued with
   handoff replies**, and ends **Completed** with a final answer that uses the review.
   (Criterion 1.)
3. On the handoff card choose **Open worker session**: a **Handoff worker** session on Claude
   Code shows the request it was given ("Asked by Codex through Plenipo Liaison") and its
   review. **Open requester session** goes back.
4. **Activity** → the Codex task: **Delegation** lists both tasks; the trail shows "Handoff
   requested → claude-code", "Reply received: Completed", "Continued with 1 handoff reply", and
   the result. (Complete Ledger trail.)
5. Reverse: Claude Code with handoffs allowed → _"Draft a five-item release checklist for a
   desktop app. Ask codex to review it for anything missing, then give the final checklist."_
   Same expectations, with Codex as the reviewer. (Criterion 2.)
6. Optional: start step 2 again and choose **Cancel turn** while it waits — the turn ends
   **Cancelled** and the reviewer's session stops too.

Things only real CLIs can confirm (report anything odd): that each CLI follows Liaison's
instructions and writes a valid `plenipo-handoff` block when asked (an invalid block is refused
and the worker told why, so the task still finishes), and how long a round trip takes.

## Out of scope

Automatic organizational delegation, arbitrary peer-to-peer runtime access, cross-machine
messaging, role-based routing (Phases 5–6), capability grants (Phase 7).
