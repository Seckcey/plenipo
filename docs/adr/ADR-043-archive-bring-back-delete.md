# ADR-043: Archive, bring back, and delete for good — agents, departments, and projects

- **Status:** Accepted (by the owner, 2026-09-27, as recommended). Amended by
  [ADR-045](ADR-045-experience-and-the-workforce.md): deleting for good first offers to save
  experienced agents to the Workforce.
- **Date:** 2026-09-27
- **Phase:** 17
- **Carries out:** ADR-039 (the owner's notes) §2.1, which amended ADR-009 (the organization) §1:
  "archived, never deleted" became "archive, then delete for good"
- **Amends:** ADR-009 §1 and §3 (a department can now be archived; "Remove department" gives way
  to archive, then delete for good)

> **On screen** (ADR-010, plain words and rank names): **Archive**, **Bring back**, **Delete for
> good**, and the **Archived** list. An agent is a position on the chart. This record keeps the
> plan's words, which are also the code's.

## In short

You can archive agents, projects, and departments, see them in an Archived list, bring them
back, or delete them for good. Deleting for good asks you first, is refused while anything is
still working, and leaves a short note in the Ledger so older history still shows who did what.
Nothing on your disk is deleted. Accepting this record means building it as written below.

## Context

The owner decided (ADR-039 §2.1): archive with the ability to bring back, and delete for good
from the archive, after confirming; "the Ledger keeps a short record in its place — name, role,
and dates — so older activity still reads correctly"; "nothing with unfinished work can be
archived or deleted, as today". Phase 17 of `ROLLOUT_PLAN.md` puts the Archived list in the
organization's List view (the canvas drawer is Phase 18) and says: "Delete for good removes the
item and its settings. The Ledger keeps a short record in its place (ID, name, role, dates,
'deleted by the owner') so older activity still shows who did it. Refused while anything has
unfinished work. Recorded as its own event."

Today (v1.9.0, Ledger layout 8):

- **Agents (positions)** are archived, never deleted: a Ledger rule stops any change to an
  archived position and any deletion. Archiving retires its agent and ends its reviewer, QA, and
  security assignments. There is no way back.
- **Projects** are archived with their whole team; there is no way back and no date recorded.
- **Departments** cannot be archived: they can be marked inactive ("takes no new projects"), or
  removed — a real deletion — only when they have no projects at all.
- **History points at them by ID.** The Activity trail copies names into each event, but task
  pages, objective results, notices, and "who did it" look positions and projects up by ID.
  Tasks and events are never deleted, so a project or an agent with any history cannot be removed
  from the Ledger without breaking those links.

## Decision

### Archive

1. **Agents and projects archive as today.** An agent can be archived when it leads no one and
   has no unfinished work; a project is archived with its whole team.
2. **A department can now be archived.** It takes everything in it along — its projects, each
   with its team, and every agent under its manager — once nothing in it has unfinished work.
   **Inactive** stays as it is ("takes no new projects").
3. **Plenipo remembers what went together.** Each archived item notes what it was archived with
   (on its own, with a project, or with a department), whether a full-time position had an agent,
   and the reviewer, QA, and security assignments that ended.

### The Archived list

4. **A new Archived tab in the organization's List view** lists archived departments, projects,
   and agents: what each was, when it was archived, what went with it, and **Bring back** and
   **Delete for good**. Selecting one shows the same two buttons in its panel's Manage tab. (The
   canvas's drawer and trash can are Phase 18.)

### Bring back

5. **It comes back as it was.** An agent returns to the same place with all its settings — title,
   role, specialty, AI tool and model, effort, rule, and learning. A full-time position that had an
   agent gets a new one; its conversation starts fresh. The reviewer, QA, and security
   assignments that archiving ended come back when the other side is active too.
6. **A project comes back with its team; a department with everything archived with it.**
   Things archived with a project or a department come back with it, not on their own.
7. **Refused, with the reason and what to do,** when its lead, project, or department is still
   archived ("Bring back the Website project first"), when its title is now taken on that team,
   or when its fixed AI tool is no longer allowed by the project. Each is recorded:
   `org.position_restored`, `org.project_restored`, `org.department_restored`.

### Delete for good

8. **Only archived items, only after you confirm.** The confirmation names everything that goes
   ("Delete the Website project for good, with the 6 agents archived with it?") and says it cannot
   be undone.
9. **Refused while anything has unfinished work** — any task of the item, or of anything going
   with it, that has not finished.
10. **Its settings are removed:** an agent's AI tool and model, effort, rule, learning setting,
    and specialty; a project's folder, repository address, allowed AI tools, permission limit,
    and description; a department's description, rule, and permission limit.
11. **A short record stays in its place,** in the same Ledger row: its ID, name, role (for an
    agent), when it was made, archived, and deleted, and "deleted by you". Task pages, results,
    notices, and the Activity trail keep showing who did what. Deleted items leave the Archived
    list and every list of the organization.
12. **A project or department takes along what was archived with it,** each leaving its own
    short record (the owner's choice, 2026-09-27). Before anything goes, Plenipo offers to save
    the experienced agents to your Workforce (ADR-045).
13. **Its name can be used again.** A deleted project or department is renamed "Website
    (deleted)" (with a number if that is taken), because project and department names must be
    unique; the event keeps the original name. An agent keeps its title, which only has to be
    unique among active teammates.
14. **Recorded as its own event:** `org.position_deleted`, `org.project_deleted`, and
    `org.department_deleted` (the event today's Remove department records, now with
    `forGood: true`, the original name, and what went with it).
15. **Nothing on your disk is deleted.** The project's folder and its working copies stay on this
    PC; Plenipo only forgets them. Deleting your files is not what was asked, and it would have to
    go through Guard.
16. **Remove department is replaced** by Archive, then Delete for good, so every deletion leaves
    its short record.

### In the Ledger

17. **Ledger layout 10:** `deleted_at` on positions; `archived_at` and `deleted_at` on departments
    and projects. The rule that an archived position never changes now allows exactly two
    changes — being brought back, and becoming a short record — and a short record never changes
    again. Rows are still never removed. A backup is made before the layout changes, as for every
    layout change.

## Consequences

- **History stays readable:** every old task still points at a row with the right name.
- **Delete for good is permanent,** so it always asks, and the Ledger's backups are the only way
  back (Diagnostics → Restore).
- **A brought-back full-time agent starts a new conversation,** because its old agent retired
  when it was archived.
- **Short records stay in the Ledger forever.** They are small (an ID, a name, dates).
- **Your files are never touched** by deleting; you delete folders yourself if you want to.

## Alternatives considered

- **Really remove the rows.** Rejected: tasks and events are permanent and point at them, and
  every page that shows "who did it" would lose the name.
- **A separate table of short records.** Rejected: every page would need a second lookup, and a
  missed one would show a blank name.
- **Keep the original name on a deleted project or department.** Rejected: the name could never
  be used again.
- **Bring back an agent without the project archived with it.** Rejected: it would come back
  reporting to an archived supervisor.
- **Delete the project's folder too.** Rejected: not asked for, and it touches your files.
