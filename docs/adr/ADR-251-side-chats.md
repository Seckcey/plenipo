# ADR-251: Side chats with a manager or supervisor

- **Status:** Accepted (the owner, 2026-10-03: "We need a way to open side chats to ask each agent
  a question if I don't want to go through the chain of command. Mainly with managers and
  supervisors while they wait for workers to complete their tasks."; item 3.5 of
  [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md), with the owner's answer 4: answer only).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 3 (item 3.5)
- **Number:** after Phase 25's block (ADR-190 to ADR-199), like ADR-250.

> **On screen** (ADR-010, plain words and rank names): **Ask a question** in the details panel
> ("A side chat: it answers from what it knows, and its work goes on.") and on the Worker page. The
> box is "Ask Website Supervisor a question", with **Your question** and **Ask**. In Workers the
> conversation is "Side chat with Website Supervisor".

## In short

The only way to talk to a full-time agent was to give it an objective, and a busy agent (one
waiting on its team included) said no.

**Accepting this record means:**

1. **Ask a question** works for any hired full-time agent (a VP, a manager, a supervisor, or a
   full-time worker), while it works or waits. An on-call position has none: it has no
   conversation of its own.
2. **A new conversation, briefed.** The side chat runs on the agent's own AI tool and model. Its
   first message says who it is, what it is doing now ("working on: …" or "waiting for your team's
   replies on: …"), and its last four objectives and answers, from Plenipo's own record of its
   conversation, then your question.
3. **Answer only** (answer 4): no tools, and its hand-off blocks are not read. Nothing said there
   changes the agent's work, and its real conversation is never touched.
4. It counts toward your plan's usage and toward Free's three at once, like any other worker. It is
   refused while Stop all work holds the work. Follow-ups go to the same side chat in Workers.

## Decision

- Each side chat starts fresh with the briefing. Continuing a copy of the AI tool's own
  conversation (Claude Code's `--fork-session`, and Codex's, Grok's, and Kimi's forks once checked)
  is not built yet: each AI tool would need its own way to start a copy. The briefing works the same
  on every AI tool.
- Plenipo gives tools only to an organization member's conversation; a side chat is marked as a
  side chat, never as a member's, so it gets none.

## Consequences

- You can ask a busy supervisor "how far along is it?" without stopping or redirecting it.
- The answer knows only what the briefing tells it, not every detail of the agent's conversation.

## Alternatives considered

- **Ask the agent itself, in its own conversation.** Rejected: it is busy, and a question in its
  conversation would change what it does next.
- **Give the side chat read-only tools.** Rejected by the owner (answer 4): answer only.
