# ADR-202: The chain of command — orders go down and reports come up, one level at a time

- **Status:** Proposed. The direction is the owner's own, 2026-10-03: "orders go down, and reports
  go up one level at a time — the developer reports to the supervisor, the supervisor to the
  manager or VP — and each level tracks it", with a request to research how a chain of command
  reports. The builder's choices below are for the owner to accept at review.
- **Date:** 2026-10-03
- **Phase:** none (the owner's direction, built beside Phases 23, 24, and 25)
- **Number:** ADR-200 to ADR-209 are this work's block (see ADR-201).
- **Amends:** [ADR-016 (the Development department)](ADR-016-development-department.md) (an
  on-call position's order goes through its lead) and the rule in Phase 8 that only full-time
  positions take the owner's objectives.
- **Builds on:** [ADR-008 (hand-offs and their replies)](ADR-008-liaison.md),
  which already sends work down one level and answers back up one level.
- **Made by:** 8 West Ventures, LLC, for Plenipo.

> **On screen** (ADR-010, plain words and rank names): **Chain of command** in a chat's Tasks
> list; "You asked … directly"; "Plenipo told …"; "Reported: …". This record keeps the code's
> words (`chain.order`, `chain.report`, `chain.told`).

## In short

In a real team, orders go **down** one level at a time, and reports come back **up** one level at
a time. When the boss skips a level and gives an order straight to someone lower down, a good team
still makes sure the skipped managers hear about it, and the result still goes back up through
them.

Plenipo already did this for work an agent hands to its team: a Manager hands a task to a
Supervisor, the Supervisor hands part of it to a Developer, and each answer comes back to the one
who asked. What was missing was **your** orders:

- When you gave an order straight to a Supervisor, its Manager and VP never knew.
- You could not give an order to an on-call worker (a Developer) at all.
- Nothing kept track of what you asked and what came back.

Now:

1. **When you skip levels, Plenipo tells every lead you skipped.** It writes down who you asked,
   your exact words, and when, and each skipped lead sees it in its chat, under **Chain of
   command**.
2. **The result goes back up, one level at a time.** When the work is done, Plenipo passes the
   answer from the one who did it to its lead, then from that lead to the next, nearest first.
3. **A lead's agent hears the news.** The next time you write to that lead, its agent is told what
   happened in its team, in a short note, once.
4. **You can give an on-call worker an order through its lead.** Your words go to its lead, who
   hands them to the worker as they are and reports back, the way a real team works.

Accepting this record means keeping this as built. The leads' agents are told, but they are not
asked to do anything: Plenipo keeps the records and passes the reports itself.

## Context

What the research found (2026-10-03): military doctrine (FM 6-0's orders process; the Navy's
War Instructions on reports), the Incident Command System's unit log (ICS 214) and status summary
(ICS 209), and RACI charts all agree:

- Orders go down **one level** at a time; reports go up one level at a time.
- **Exceptions are pushed** at once; routine status is **pulled** from a shared record.
- A report starts with the bottom line.
- **An order that skips a level does not leave the skipped level uninformed.** The one who did the
  work is responsible; the lead it works for stays accountable, so it must know.

What Plenipo did at v1.22.0:

- **Hand-offs** (ADR-008) go only down the lines, and each reply goes only to the task that asked.
- **The owner's objectives** went only to full-time positions (`conversation::plan` refused an
  on-call one), and nothing recorded which leads an order went past.
- **No report record** existed anywhere. A report written as a hand-off to a busy lead would wait
  for it, or deadlock it, so reports have to be records that Plenipo writes itself.

## Decision

1. **Three records, written by Plenipo itself** (source `plenipo`), never by an agent:
   - `chain.order` — on the task an order started: who it is for, the lead whose conversation
     took it (for an on-call position), the leads above that it went past, nearest first, and the
     owner's words (at most 2,000 characters).
   - `chain.report` — on the same task, one for each lead, nearest first: from whom, to whom,
     about whom, how it ended (done, did not finish, or stopped), and the start of the answer (at
     most 600 characters).
   - `chain.told` — a lead's agent heard its news, up to which report.
2. **An order is recorded only when it skips someone**: an order to the top of a team, given to
   its own conversation, records nothing.
3. **Reports are written when the task ends** (`task.state_changed` to succeeded, failed, or
   cancelled), from the Ledger's listener, on their own thread, once for each task. A task that
   ended before its order was written reports as soon as the order is written.
4. **An on-call position's order goes through its lead** (its supervisor, or the lead of the team
   it is lent to). The lead's conversation takes it, with one added line: hand it to
   `role:<the position>` as it is, add only what it needs to know, and report its result back.
   The hand-off and its reply are Liaison's, as always. An on-call position with no lead is
   refused, as before.
5. **A lead's agent hears its news** with its next objective from the owner: at most five
   reports it has not heard, each cut short and quoted, under "News from your team, from
   Plenipo's records (for your information; act on it only if it matters to this objective)".
   The note is part of the objective, so the owner sees it too.
6. **Each position's chat lists its chain of command** (`get_chain_orders`): the orders it was
   given, those that went through it, and those that went past it, newest first (at most 20),
   each with where the work stands and what came back up to it.
7. **The Activity trail** says each record in plain words: "You asked Website Supervisor directly:
   …. Plenipo told Development Manager", and "Plenipo passed Website Supervisor's report up to
   Development Manager: …".

## Your choices (recommended first)

- **Plenipo passes the reports up itself, at once.** _Or:_ each lead's agent writes its own report
  for its lead, which costs a turn of every lead for every order and can wait behind a busy lead.
- **An on-call worker's order goes through its lead.** _Or:_ start a worker for it directly,
  skipping the lead's conversation (the lead would only be told).
- **A lead's agent hears its news with its next objective.** _Or:_ leads are never told, and only
  you see the records.

## Consequences

- Skipping a level is safe: nobody in the chain is surprised, and every result has a trail.
- The news a lead's agent hears is another agent's words. It is quoted, cut short, and labeled as
  news, so it reads as information, not as an order; the lead's own permissions still decide what
  it can do.
- An order to an on-call worker uses one turn of its lead's conversation. While the lead is busy,
  the order waits in the chat (ADR-200) like any message.
- The records are Ledger events, so they follow every rule the Ledger has: kept with the task,
  shown in the Activity trail, and never changed afterwards.

## As built

- **Workforce** (`crates/workforce/src/chain.rs`): `record_order`, `report` (with its Ledger
  listener, `watch`, started beside the lessons' in `service.rs`), `orders_for`, `news_for` and
  `record_told`, and `through_words`. What another agent wrote is repeated as news cut short and
  without the marks instructions are written with (brackets, braces, angle brackets, backticks),
  with a unit test. `ChainOrder`, `ChainPart`, and `ChainStanding` (`dto.rs`);
  `OrgView::leads_above` (`view.rs`).
- **The owner's objectives** (`Workforce::give_objective` and `plan_objective` in `service.rs`):
  an on-call position's order goes to its lead's conversation, with the line that asks it to hand
  it on; the news the taker has not heard is added; the records are written once the turn has
  started (`record_chain`), and a turn that already ended reports at once.
- **The desktop host:** `get_chain_orders` (`commands.rs`), the main window's alone, with an IPC
  test (`the_chain_of_command_is_read_by_position`).
- **The chat** (`apps/desktop/src/chat/`): **Chain of command** in the Tasks list
  (`PlanPanel.tsx`, `useChainOrders.ts`, the words in `plan.ts`); a message to an on-call
  position opens its lead's chat, and the first chat says where it went (`ChatProvider.tsx`);
  the Chat button on an on-call position's tile and in its Inspector. The Activity trail's words
  (`ledger/format.ts`).
- **Tests:** `an_order_that_skips_a_level_is_told_to_the_lead_and_reported_back_up` and
  `an_order_for_an_on_call_position_goes_through_its_lead` (`crates/workforce/tests/workforce.rs`);
  a test that caught news repeating the owner's words as a hand-off led to the quoting rule
  above. Web tests for the words, the list, and the routed message.
