# ADR-208: Talking to an agent is direct — your words go straight to it, not through the Liaison

- **Status:** Proposed. The direction is the owner's own, 2026-10-05 (Phase 25, I5): "Talking
  directly to an agent in its chat must not go through the Liaison. The Liaison is only for agents
  from different labs working together." His answers to the design (D1, D1b, D2 below) are the
  same day's.
- **Date:** 2026-10-05
- **Phase:** 25 (I5)
- **Amends:** [ADR-202 (the chain of command)](ADR-202-the-chain-of-command.md), point 4 (an
  on-call position's order goes through its lead), and
  [ADR-008 (hand-offs and their replies)](ADR-008-liaison.md), whose Liaison no longer carries
  the owner's own words.
- **Made by:** 8 West Ventures, LLC, for Plenipo.

> **On screen** (ADR-010, plain words and rank names): nothing says "Liaison" in an agent's chat.
> Its hand-offs to its team are "hand-offs", and the chain of command keeps its words (ADR-202).
> This record keeps the code's words.

## In short

When you type in an agent's chat, your words go **straight to that agent**:

1. **Nothing in between.** Plenipo gives your words to the agent's own conversation. The Liaison
   (the part of Plenipo that passes work between agents) is not in the way, and the agent's
   instructions name Plenipo, not the Liaison.
2. **An on-call worker too.** Typing in an on-call worker's chat talks to that worker, not to
   its lead. That conversation stays open for your follow-ups, like a full-time member's, until
   you end it.
3. **The Liaison stays for every hand-off between agents**, from any lab: a Claude agent handing
   work to another Claude agent goes through it, as does a Codex agent handing work to a Claude
   agent. That is what lets you see that work, stop it, and read it afterwards.

Accepting this record means keeping this as built.

## Context

What Plenipo did at v1.26.0, when you wrote in an agent's chat:

- **To a full-time agent** (a Supervisor, a Manager, a full-time Developer): Workforce asked the
  Liaison to start the turn (`resume_member_session`, `start_member_session`). The Liaison put
  its own instructions first ("[Plenipo Liaison — instructions] … you may ask another AI worker
  for help through Plenipo Liaison …"), tagged the task as a Liaison workflow, and read every
  answer for hand-offs. No Liaison message was made for your words, but the agent was told it
  worked through the Liaison, and could say so.
- **To an on-call worker** (a Developer who works when its lead hands it work): your words never
  reached it. By ADR-202 point 4 they went to its lead's conversation, with a line asking the lead
  to hand them on; the lead handed them to a newly staffed worker through the Liaison, and the
  screen said "Sent to Cloudline Supervisor, who hands it to Senior Developer and reports back".
  The worker's own chat showed the lead's request, and you could not write in it.

The owner's report (2026-10-05, B5): the Senior Developer did the work and sent it back to its
supervisor, and opening the Senior Developer's chat did not show that conversation. Talking to an
agent should be talking to that agent.

## Decision

1. **The owner's words go straight to the agent's conversation.** Workforce gives them to the
   runtime itself (`runtime.resume_session_with`, `start_session_with`), as a task the owner asked
   for. The Liaison only supplies, through a thin helper:
   - the worker's place on the job (Free runs three at a time, ADR-113), given back once the
     turn has started (`Liaison::owner_place`);
   - its instructions (`Liaison::direct_turn`): who it is, its team, and how to hand work on,
     under **"[Plenipo — instructions]"**, the same words as the Liaison's first message except
     its name. The same instructions give the same number (ADR-044), so a conversation that has
     them gets the short reminder;
   - the records that let its hand-offs, if it makes any, form a workflow: the task's tag
     (correlation and depth), and a member's conversation that the Liaison reads for hand-offs.
     These are records only; no message is made for the owner's words.
2. **An on-call worker's chat talks to that worker** (D1). Each message staffs a worker the way a
   hand-off does (the same routing and hire), with the owner as the one who asked; no lead in
   between and no Liaison request. The worker leaves when its answer is done, as one staffed for a
   hand-off does, so Free's places and the worker counts work as before. ADR-202's records stay:
   every lead above is told, and the result goes back up (points 1–3, `chain.order`,
   `chain.report`). Only point 4's routing changes.
3. **That conversation stays open** for follow-ups, like a full-time member's, until the owner
   ends it (D1b): the same runtime session resumes for the next message, so its new worker
   remembers the earlier ones. Ending the chat closes the session; the next message starts a new
   one. A lead's own hand-offs to that position still staff separate workers, in their own
   sessions.
4. **The Liaison carries every hand-off between agents, from any lab** (D2): requests, starting
   the worker, its permissions, depth limits, replies and delivering them, check-ins, sending an
   answer back to check, stopping team work, and its records. Its turn hook reads the answers of
   the owner's turns only for hand-offs.

## Your choices (recommended first)

- **One hand-off path for every lab** (D2, chosen). _Or:_ teams from the same lab skip the
  Liaison and use their AI tool's own helpers (Claude Code's subagents, say); Plenipo could then
  not see, stop, limit, or record that work, and B5 could not show it.
- **An on-call worker's chat stays open** (D1b, chosen). _Or:_ each message starts a new
  conversation, which forgets the earlier ones, as a hand-off's does.
- **A worker for each message, in the one open conversation** (chosen). _Or:_ one worker for the
  whole chat, which needs a new kind of worker record that is not tied to a task, and a new way
  to retire it.
- **The task's workflow tag is written when the owner's turn starts**, as a record that the agent
  never sees. _Or:_ write it with the first hand-off. Several of the Liaison's checks look tasks up
  by that tag (what a request may reference, the tree of a workflow), so writing it later would
  change all of them for no visible gain.

## Consequences

- Your words reach the agent you are talking to, and its instructions no longer name the Liaison.
  Conversations that started before keep the instructions they were given until their next full
  set.
- Messages that come from the Liaison into the same conversation keep its name: the replies to
  the agent's hand-offs, a check-in on its team, and an answer sent back to check. They are the
  Liaison's own, between agents (D2).
- An on-call worker's open chat costs nothing while it waits: a worker is staffed only while it
  answers. The owner ends the chat when done.
- An on-call position that is lent to another team (ADR-054) takes the owner's words only as the
  loan allows, and says so.
- No new screen words: the Liaison's name appears nowhere in a chat.

## Built in pieces

1. **The direct brief and `give_objective`'s path** (this record's first pull request):
   `context::direct_brief` and `DIRECT_HEADER` (`crates/liaison/src/context.rs`);
   `Liaison::owner_place`, `Liaison::direct_turn`, and `members_conversation`
   (`crates/liaison/src/service.rs`); `Workforce::give_objective` gives the owner's words to the
   runtime itself (`crates/workforce/src/service.rs`). The fake AI tool reads the new header as
   the Liaison's, and keeps each message's first line (`headers.log`) for the tests. Tests:
   `the_owners_words_to_a_member_name_plenipo_not_the_liaison` (`context.rs`) and
   `the_owners_words_go_straight_to_the_agent` (`crates/workforce/tests/workforce.rs`).
2. **An on-call worker's direct chat** (D1, D1b): `conversation::direct_plan` finds the
   position's open direct chat (`Ledger::open_direct_session`: an open session with
   `metadata.directChat` and the position in `metadata.workforce`), or routes a new one;
   `Workforce::give_objective` records the owner's task and its worker together
   (`Ledger::create_owner_task_with_worker`) and runs that task in the conversation
   (`TurnTask::Existing`, numbered by `Ledger::begin_task_turn`). While the conversation is busy
   nothing is recorded; a task whose turn did not start is cancelled, and its worker leaves.
   `plan_objective` no longer reroutes to the lead, and `chain::through_words` is gone. The chat:
   no "Sent to its lead" note, an on-call position's chat is its own, and "End this chat" closes
   the session. Tests: `talking_to_an_on_call_worker_is_direct` (`workforce.rs`, replacing
   `an_order_for_an_on_call_position_goes_through_its_lead`), and the Ledger's
   `an_owners_task_for_an_on_call_position_comes_with_its_worker` and
   `an_on_call_positions_open_direct_chat_is_found`.
3. **Retiring** `Liaison::resume_member_session` and `start_member_session`, which only tests
   use after piece 1.
