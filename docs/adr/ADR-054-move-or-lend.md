# ADR-054: Move or lend an agent to another team

- **Status:** Proposed (waiting for the owner)
- **Date:** 2026-09-28
- **Phase:** 18
- **Carries out:** ADR-039 (the owner's notes) §2.2 — "dropping an agent on another team moves
  it for good, as today. A menu choice lends it instead … While lent, it works under the
  borrowing project's permission limit, never its home project's."
- **Amends:** ADR-009 (the organization) §5 — a team's members include the agents lent to it;
  ADR-013 (Guard and the capability broker) — a task's own team decides its permission limit

> **On screen** (ADR-010, plain words and rank names): **Move here**, **Lend for one
> objective**, **Lend until I send it home**, **Lent to the Website team**, **Send home**, and a
> **lent** line. This record keeps the plan's words ("lending is a Workforce record"), which are
> also the code's.

## In short

When you drop an agent on another team, you choose: **Move here** (it joins that team for good,
as today), or **lend** it — for one objective, or until you send it home. A lent agent stays on
your chart where it was, with a "lent" badge and a line to the team it is helping. While lent, it
takes work only from that team, and it works under that team's permission limit and in that
team's working copy — never its home project's. When the objective is done, or when you press
**Send home**, it goes back by itself. Both are recorded. Accepting this record means building
it as written below.

## Context

Phase 18 of `ROLLOUT_PLAN.md`: "move or lend: dropping an agent on another team offers Move here
(for good) or Lend for a job (one objective, or until returned); a lent agent shows a 'lent' line
and badge, and goes home by itself when done." Technical notes: "Lending is a Workforce record
(who, from which team, to which team, for what, since when). While lent, an agent takes
objectives from the borrowing team and works under the borrowing project's permission limit,
never its home project's (ADR-039 §2.2). Returning is recorded." The acceptance criteria lend "a
Security Auditor to another department for one objective" and see "it come back".

What the code does today (v1.10.0, read at `c5d1a71`):

- **A team** is worked out from the reporting lines: a lead's team is its on-call reports, the
  on-call agents that review, test, or check its work, and its full-time reports
  (`crates/workforce/src/view.rs`). An agent's department and project are the nearest department
  head and Supervisor above it. There is no membership table.
- **On-call agents** (every built-in worker role, the Security Auditor included) get a new
  worker for each task. When a lead hands one a task, the new worker's conversation carries the
  **lead's** project (`crates/workforce/src/directory.rs`), and Guard gives it that project's
  permission limit and working copy. Oversight already works this way: an auditor checking
  another team works under that team's project.
- **Full-time agents** keep one conversation across objectives. Guard reads the project from the
  conversation's own record, written when it started (`crates/capabilities/src/broker.rs`,
  `try_open`), and nothing updates it. **So a full-time agent moved to another project today
  keeps its old project's permission limit and working copy until it gets a new conversation.**
  This mapping found it; this record fixes it (§9).
- **Moving** is `move_position`, with the checks in `org/rules.ts` and the Ledger
  (`org.position_moved`).

## Decision

### Move here

1. **Move here** is today's move, named for what it does: the agent (and, for a lead, its team)
   now reports to the agent it was dropped on, with today's checks and events.

### Lend

2. **Who can be lent:** an **on-call** agent that leads no one, has nothing unfinished, and is
   not already lent — which is every built-in worker role. A full-time agent is moved, not lent:
   its one conversation remembers its home project's work, and lending it would carry that
   memory, and that project's details, into another project.
3. **To whom:** a full-time lead of **another** team (a VP, a Manager, a Supervisor, or a
   full-time agent of your own role), active. Its AI tool, if fixed, must be one that team's
   project allows, and its title must not clash with a title on that team (hand-offs find team
   members by title).
4. **For how long,** chosen in the drop menu:
   - **Lend for one objective:** the first task that team hands it ties the loan to that team's
     objective. It can take more tasks in the same objective (a check, then a second check after
     the fixes). When that objective is done — finished, failed, or cancelled — and its own work
     in it has ended, it goes home.
   - **Lend until I send it home:** it helps that team until you press **Send home**.
5. **While lent:**
   - it is on the borrowing lead's team: that lead can hand it work, and its team list says
     "lent from the Development team";
   - it is **not** on its home team: a hand-off from home is refused with the reason ("Security
     Auditor is lent to the Website team until its objective is done"), and the review, QA, or
     security assignments it holds at home wait until it is back;
   - it works **in the borrowing team**: that team's project decides its **permission limit**,
     its **working copy**, and the AI tools it may use; that team's department decides the
     **department permission limit** and the department's model and effort rule (ADR-041 §5: "an
     agent's department is the department it works in"). Its own settings travel with it: its
     role, specialty, its own rule, and its learning setting. Lessons it writes go to its role,
     for the project it wrote them in (ADR-050);
   - it cannot be moved, archived, deleted, or lent again until it is home ("Send it home
     first").
6. **Send home:** on the agent's details (Team tab) and on its "lent" badge. If it is working, it
   finishes the task it is on, takes nothing new, and goes home when that task ends ("going home
   after this task").
7. **Goes home by itself** when its objective is done (§4), or when the borrowing lead, its
   project, or its department is archived or deleted (in the same step).
8. **On the canvas:** the agent stays where it is on your chart, with a **Lent to the Website
   team** badge and a dashed **lent** line to the borrowing lead, both in the legend (ADR-053).

### A task's own team decides its limits (the fix)

9. **Guard reads the project and department from each task's own record**, which Workforce
   writes for every task, and no longer from the conversation's first record. So:
   - a **lent** agent's task names the borrowing team's project and department;
   - a **moved** full-time agent's next objective runs under its **new** project's permission
     limit and working copy — the gap found while mapping the code is closed.
     A task without a Workforce record (the Workers page) is unchanged.

### In the Ledger

10. **Ledger layout 11** adds `loans`: which agent, from which lead, to which lead (and that
    team's project and department at the time), for one objective or until sent home, the
    objective it joined, when it started and ended, and why it ended (its objective was done, you
    sent it home, or the team it helped was archived). Rows are never removed.
11. **Recorded:** `org.agent_lent` (who, from, to, for how long) and `org.agent_returned` (who,
    why, and the objective it helped with), each in the same transaction as the change.
12. **New desktop commands, the main window's alone:** `lend_agent` and `send_home`, refused from
    the sign window and from any web page, with IPC tests.

## Your choices (recommended first)

- **Lend on-call agents only** (every built-in worker role); move full-time agents. _Or:_ lend
  full-time agents too, with a second conversation for the other project — more to build, and
  two conversations per agent to keep apart.
- **"One objective" means that team's whole objective,** so the lent agent can check twice.
  _Or:_ exactly one task, then home.
- **While lent, the borrowing team's department rule picks its model and effort**, like
  everything else about where it works. _Or:_ keep its home department's rule (its own rule
  still wins either way).
- **Send home while working lets it finish the task it is on.** _Or:_ refuse until it is idle.

## Consequences

- Lending uses the path oversight already uses, so a lent agent's work is checked by the same
  Guard rules as any team member's; it can never carry its home project's permission limit into
  another project.
- The fix in §9 changes where Guard reads the project for full-time agents; tests cover a moved
  full-time agent, a lent agent, and a task with no Workforce record.
- One more place an agent can be: on its team, lent, archived, in your Workforce, or deleted for
  good. Each shows where it is.

## Alternatives considered

- **Show a lent agent on the borrowing team's part of the chart.** Rejected: the plan asks for a
  "lent" line and badge, and keeping it at home shows where it will come back to.
- **Lend with a new copy of the agent.** Rejected: it would not be the same agent, and its
  experience would split in two.
- **Keep reading the project from the conversation.** Rejected: it gives a moved agent the wrong
  permission limit.
