# Phase 4 — Implementation Checklist

**Status:** in progress on `claude/phase-4`.

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
  its request's correlation ID, message ID, child task, and destination, and a request is
  answered at most once.
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

- [ ] Liaison service (`crates/liaison`): turn hook, validation, dispatch, reconciliation
- [ ] Internal message envelope (requests and replies), persisted with dedupe keys
- [ ] Task handoff protocol (`plenipo-liaison/1`): directive format, worker instructions
- [ ] Context packet format (`plenipo-context/1`)
- [ ] Correlation IDs on messages, child tasks, and every Liaison event
- [ ] Reply routing back to the requesting task (continuation step in the same session)
- [ ] Parent-child task visualization (Workers handoff cards, Activity delegation tree)
- [ ] Codex-to-Claude handoff
- [ ] Claude-to-Codex handoff
- [ ] Ledger migration 0003 (`liaison_messages`), up/down and v2 → v3 tested
- [ ] Runtime: multi-step turns, suspend/continue, cancel while waiting, busy vs not ready
- [ ] Fake CLI handoff behaviors for tests (`plenipo-fake-agent`)
- [ ] ADR-008; architecture, configuration, setup, README updated

## Phase 4 tests (from plan)

- [ ] Codex task creates Claude child task
- [ ] Claude returns result to Codex parent
- [ ] Reverse direction
- [ ] Nested task depth limits
- [ ] Missing destination
- [ ] Failed worker
- [ ] Canceled parent task
- [ ] Duplicate message protection
- [ ] Correlation integrity

Also: restart recovery, worker-slot queueing, per-answer and per-workflow limits, capability
requests denied, forged identity fields rejected, non-handoff sessions ignore directives, IPC
boundary tests for new commands, end to end through the real app.

## Acceptance criteria (from plan)

- [ ] A Codex worker can request a Claude review through Liaison, Claude can complete the
      review, and the response appears in the originating Codex workflow with a complete
      Ledger trail.
- [ ] The reverse path also works.

CI proves both against fake CLIs; the owner verifies both with the real CLIs on Windows.

## Out of scope

Automatic organizational delegation, arbitrary peer-to-peer runtime access, cross-machine
messaging, role-based routing (Phases 5–6), capability grants (Phase 7).
