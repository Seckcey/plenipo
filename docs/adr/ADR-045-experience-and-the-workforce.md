# ADR-045: Experience and the Workforce — keeping your best agents

- **Status:** Accepted (by the owner, 2026-09-28, as written; experienced agents start out
  checked)
- **Date:** 2026-09-28
- **Phase:** 17 (added by the owner while approving ADR-043)
- **Amends:** ADR-043 (archive, bring back, and delete for good) — deleting for good first offers
  to save experienced agents; ADR-024 (workers learn from their work) — each agent's kept lessons
  count toward its experience

> **On screen** (ADR-010, plain words and rank names): **Experience** (a score), the
> **Workforce** (agents you saved to hire again), **Save to my Workforce**, and **Hire from my
> Workforce**. This record keeps the code's words (position, saved agent).

## In short

Every agent gets an **experience** score: how much it has learned and done. Agents you want to
keep can be saved to your **Workforce**, a tab on the Organization page, and hired again later
into any team, with their settings, experience, and lessons. When you delete an agent, a project,
or a department for good, Plenipo offers to save the agents whose experience is above average;
the ones you do not save are deleted for good. Accepting this record means building it as written
below.

## Context

When approving ADR-043, the owner asked (2026-09-28): "When our agents start learning their jobs
better, we need a way to score how much the agent has learned. For the agents with a lot of
experience and learned knowledge, Plenipo needs to have a Workforce tab where we can save and
store agents to. When deleting an agent or organization, if there are agents with higher than
average score, offer to save them to the user's Workforce for future use. If none are selected to
save and move to the Workforce then they get permanently deleted."

What the Ledger already knows about each agent (a position on the chart):

- **The lessons it wrote** (ADR-024): each lesson records the position that wrote it, and
  whether you kept it. Kept lessons belong to the role, so every worker of that role gets them.
- **The tasks it did:** every task records its position, and whether it finished.

Nothing scores an agent today, and nothing keeps an agent once it leaves the chart.

## Decision

### Experience

1. **Every agent has an experience score:** 10 points for each lesson it wrote that you kept (and
   still keep), plus 1 point for each task it finished. Learning is what the owner asked to score,
   so it counts the most; finished work shows it has done the job.
2. **Shown with its reasons** on the agent's Overview tab: "Experience 57 — 4 lessons you kept,
   17 tasks done. Your organization's average: 22." It is worked out from the Ledger each time,
   so nothing new is stored for it and it cannot drift.
3. **The organization's average** is taken over its agents that have finished at least one task,
   active or archived.

### The Workforce

4. **A Workforce tab on the Organization page** (in the List view, beside Archived) lists the
   agents you saved: each one's title, role, specialty, experience, where and when it worked, and
   **Hire into a team** and **Delete for good**.
5. **Saving an agent** (from the Archived list, and when deleting for good, below) moves it off the
   chart into your Workforce. The Workforce keeps:
   - its title, role, and specialty;
   - its AI settings: a fixed AI tool and model, or its own rule, and its effort;
   - its learning setting;
   - its experience: the score and its reasons, where it worked, and when;
   - a copy of the lessons it wrote that you kept.

   Its old place on the chart becomes a short record ("moved to your Workforce"), so older
   history still shows its name, as for deleting (ADR-043 §11). Recorded as
   `org.agent_saved`.

6. **Hiring from the Workforce** puts it on the team you choose, with the same checks as any
   hire (its role must fit the team; its fixed AI tool must be allowed by the project). It keeps
   its settings, and its experience carries on from its saved score. Any of its kept lessons that
   its role no longer has — and that you did not remove yourself — are added back to the role as
   kept. It leaves the Workforce. Recorded as `org.agent_hired_from_workforce`.
7. **Deleting a saved agent for good** asks first and removes it from the Workforce. The lessons
   its role keeps stay with the role.

### When deleting for good

8. **Plenipo offers to save the experienced ones.** The confirmation for deleting an agent, a
   project, or a department for good lists every agent that goes. Those whose experience is above
   the organization's average are marked "experienced" and **checked** under "Save to my
   Workforce", so nothing experienced is lost by accident. You can check or uncheck any agent.
   Checked agents move to your Workforce; the rest are deleted for good.
9. **"Organization"** here means a department or a project, the parts of your organization you can
   delete in this phase. Deleting a whole organization comes with more than one organization
   (Phase 21), with the same offer.

### Where it is kept

10. **Ledger layout 9** (the same change as ADR-042 and ADR-043) adds a `saved_agents` table:
    ID, title, role, specialty, settings, experience, lessons (copies), the position it came
    from, and when it was saved. It is included in backups and in exports.
11. **The Workforce is yours, not one organization's.** Until Phase 21 adds more organizations, it
    lives in this organization's Ledger; Phase 21 moves it where all your organizations can hire
    from it.

## Consequences

- **Your best agents are never lost by a cleanup:** deleting for good always shows who is
  experienced, and saves them unless you say otherwise.
- **The score is simple and explained,** so you can trust it and argue with it. It can be refined
  later (for example, counting reviews an agent passed) with its own record.
- **Lessons still belong to roles** (ADR-024): a saved agent carries copies so hiring it back can
  restore what its role lost, but its role keeps learning from everyone.
- **One more place for agents:** on the chart, archived, in your Workforce, or deleted for good.
  Each shows where it is.

## Alternatives considered

- **Lessons that belong to one agent only.** Not chosen: the plan keeps lessons per role
  (ADR-024), so every worker of a role benefits; the copies give the saved agent its own record.
- **Score only the lessons.** Not chosen: an agent that has done a lot of work but written few
  lessons would look new.
- **Nothing checked in the delete dialog.** Not chosen: one click could lose an experienced agent
  for good.
- **The Workforce as its own page in the menu.** Not chosen: it sits beside the Archived list,
  where agents leave and come back.
