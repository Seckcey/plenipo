# ADR-008: Plenipo Liaison — message bus and cross-provider handoffs

- **Status:** Proposed
- **Date:** 2026-09-26
- **Phase:** 4

## Context

Phase 4 lets agents and managers hand tasks to other agents "through Plenipo without directly
controlling one another". The plan requires a Liaison service, an internal message envelope,
a handoff protocol, a context packet format, correlation IDs, reply routing, parent-child task
visualization, and handoffs in both directions between Codex and Claude Code. Liaison must
validate sender identity, create child tasks, resolve destinations, attach minimal context,
dispatch to the runtime, record all events, return normalized results, update the parent
task, and surface blockers. Out of scope: automatic organizational delegation, arbitrary
peer-to-peer runtime access, cross-machine messaging.

Constraints from earlier decisions: workers have no tools and no capabilities until Guard
(Phase 7; ADR-007 §5); the Ledger is the system of record and every change writes its event
in the same transaction (ADR-006); model output is untrusted and must never change policy
(plan §3.1); no silent provider switching (ADR-003).

## Decision

1. **Agents request handoffs in their answer; Liaison decides.** A worker in a session that
   allows handoffs ends its answer with one fenced block per request (at most 3):

   ````text
   ```plenipo-handoff
   {"to": "claude-code", "objective": "Review the function above for correctness.",
    "acceptanceCriteria": "List concrete defects, or say it is correct.",
    "context": [{"kind": "answer"}], "priority": 2}
   ```
   ````

   Fields: `to` (required), `objective` (required, ≤ 4,000 characters),
   `acceptanceCriteria` (≤ 2,000), `context` (≤ 6 references), `artifacts` (artifact IDs),
   `capabilities` (names from the plan's capability list), `priority` (0–4). Anything else —
   including identity fields such as `source`, `messageId`, `correlationId`, or
   `parentTaskId` — rejects the request. A block over 16 KiB, invalid JSON, or an unterminated
   block is rejected. The worker learns the protocol from instructions Plenipo prepends to each
   owner objective in such a session (`plenipo-liaison/1`; restated every time because a
   provider may compact earlier turns away, and destinations may have changed); the recorded
   objective stays the owner's text. Tool calls or MCP were rejected for Phase 4: they would grant agents a
   capability before Guard exists.

2. **Identity and destinations.** The source of a request is the session and task Plenipo
   ran, taken from its own records; Liaison checks that the task is running in that session and
   that the session allows handoffs. Destinations: a runtime ID (`claude-code`, `codex`,
   optionally prefixed `runtime:`) means a new ephemeral worker session on that runtime, with
   its default model. Existing sessions (`session:<id>`) are never addressable by agents.
   `role:<name>` is part of the envelope format, but role routing needs the Workforce engine
   and model policy (Phases 5–6), so in Phase 4 a role is a missing destination. An unknown,
   uninstalled, or signed-out destination is never replaced by another provider.
3. **Envelope.** Requests and replies are persisted in a new `liaison_messages` table
   (migration 0003) with: message ID (UUID), correlation ID, kind, `in_reply_to`, the requesting
   (parent) task, the child task, source and destination addresses, state, a dedupe key, and
   the full envelope JSON (objective, acceptance criteria, context references, artifact
   references, capability request, priority, timestamp; replies carry the child's normalized
   result). Request states: `accepted → dispatched → answered`, `accepted | dispatched →
cancelled`, or `rejected`. Reply states: `pending → delivered | discarded`.
4. **Context packets.** Context is passed by reference and resolved by Liaison into a capped
   `plenipo-context/1` packet: the requester's own answer (`{"kind": "answer"}`, the text
   outside handoff blocks), tasks and artifacts **of the same workflow** (`task`, `artifact`
   references resolve to objective/result summaries and path/URI/hash — never file contents),
   and short inline excerpts (≤ 8,000 characters each, 24 KiB in total). The packet is
   rendered into the child's prompt, clearly delimited as data from another worker, and
   recorded with the child task.
5. **Correlation.** Every owner task in a handoff session starts a workflow with a new
   correlation ID (task metadata `liaison.correlationId`, depth 0). Child tasks inherit it with
   depth + 1. Every message and Liaison event carries it. A reply is accepted only if its
   `in_reply_to`, correlation ID, and child task match the request (its destination is always
   the requester), and only once (unique per request); a mismatched reply is refused and
   recorded (`liaison.reply_refused`) even when the request is already settled.
6. **Waiting and continuing (multi-step tasks).** When a step's answer contains requests, the
   step's result, the requests (accepted ones create child tasks in `queued`), rejections, and
   the parent's move to `blocked` ("waiting for N handoff replies") are one Ledger
   transaction. The session stays reserved for that task: no follow-ups, no close. When every
   request is answered or rejected, Liaison resumes the same provider session with a message
   listing the replies; that run is the next _step_ of the same task (a second execution),
   recorded with `blocked → running` and the replies marked delivered in one transaction.
   The task then finishes normally or hands off again. A child's final answer is its reply,
   whatever its outcome (completed, failed, usage limit, crashed, cancelled, …), so blockers
   reach the requester instead of hanging it.
7. **Reconciliation.** Liaison listens to committed Ledger events and runs one serialized
   reconcile pass: (a) answer open requests whose child task finished; (b) cancel open
   requests whose parent task finished, stopping the child (queued, running, or itself
   waiting — cascading down the tree); (c) dispatch accepted requests when a worker slot is
   free; (d) deliver a waiting task's replies when none of its requests is still open;
   (e) discard pending replies to tasks that no longer wait; (f) close finished handoff
   workers' sessions. Each action is guarded by the recorded state (compare-and-set), so
   repeated or concurrent triggers are harmless. A child's dispatch is recorded in the same
   transaction that starts its turn; a runtime at capacity (`busy`, which also covers a turn
   still being recorded or being cancelled) is retried later, any other refusal fails the
   child with the reason, which becomes its reply.
8. **Duplicate protection.** Each request has a dedupe key (requesting task, step, canonical
   request JSON) with a unique constraint: the same request twice in one answer, or a step
   processed twice, creates one child. Message IDs are unique; a request can be answered once;
   replies are delivered once.
9. **Limits.** Depth 3 (owner task = 0), 3 requests per answer, 5 reply rounds per task, 12
   accepted handoffs per workflow. Requests over a limit are rejected with the reason, which
   the requester receives like a reply so it can do the work itself; past the round limit the
   task finishes instead of waiting again. Children use the runtime's global cap (4 running
   turns) and 30-minute turn limit.
10. **Capabilities.** Capability requests are validated, recorded, and denied: Phase 4 grants
    none (Guard, Phase 7). Children keep the Phase 3 posture (Claude Code: no tools, no MCP;
    Codex: read-only sandbox) in their own empty workspace.
11. **Events.** On the requesting task: `liaison.handoff_requested`, `liaison.handoff_rejected`,
    `liaison.duplicate_ignored`, `liaison.reply_received`, `liaison.replies_delivered`,
    `liaison.reply_discarded`, `liaison.handoff_cancelled`, and when something is refused,
    `liaison.sender_rejected`, `liaison.reply_refused`, or `liaison.delivery_failed`. On the child: `liaison.handoff_received`
    (with the context packet summary and capability decision), `liaison.dispatched`,
    `liaison.dispatch_failed`, `liaison.reply_sent`. Plus the usual task, execution, and agent
    events, all in order on each task's trail.
12. **Restart.** Turns of a workflow that were running or waiting when Plenipo stopped are
    recorded as interrupted (ADR-007 recovery); Liaison then answers their open requests,
    cancels handoffs that were still queued, and discards replies nobody waits for. Nothing is
    dispatched or resumed automatically at startup.
13. **Code layout.** New crate `crates/liaison` (`plenipo-liaison`, planned by ADR-004). The
    Ledger-backed runtime stores (executions, sessions, turns) move from the desktop app into
    it, because Liaison records handoffs in the same transactions as turn state and its tests
    need the real stores. The runtime gains only provider- and Liaison-neutral extension
    points: separate recorded objective and prompt, preassigned session IDs, opaque session
    metadata, multi-step turns, a turn-end hook that may suspend a task, continuing a waiting
    task, and a distinct "busy" error.

## Consequences

- Agents collaborate through recorded, validated messages; neither controls the other's
  process or session, and the owner sees every request, reply, and child task.
- The protocol relies on models following written instructions. A model that never emits a
  valid block simply does not hand off; an invalid block is rejected and explained to it.
  Real-CLI behavior is part of owner acceptance.
- Replies are other agents' text. They are delimited and described as untrusted data, but a
  model can still be influenced by them; with no tools or capabilities the effect is limited
  to text until Guard.
- A task can now have several executions (steps). The Workers view and the Ledger show them
  in order; the UI splits live activity by step.
- Handoffs cost additional provider turns on the owner's subscriptions; limits bound them.
- Interrupted workflows are not resumed automatically; the owner can follow up in the session.

## Alternatives considered

- **Tool call or MCP server for handoffs** — natural for agents, but it grants a capability
  (`mcp.invoke`) before Guard can govern it. Revisit in Phase 7.
- **Continuation as a new turn/task** — would leave the requesting task finished while its
  work continues elsewhere, splitting one objective across unrelated tasks; waiting and
  continuing the same task keeps the tree and trail truthful.
- **Direct session-to-session messaging** — explicitly out of scope (peer-to-peer access).
- **Role routing now** — role templates and model policy belong to Phases 5–6; a stopgap
  role→runtime table would pre-empt them.
- **In-memory message queue** — faster to build, but restart and duplicate behavior would
  depend on process memory; the Ledger-backed messages plus reconciliation are durable and
  idempotent.
