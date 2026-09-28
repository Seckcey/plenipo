# Phase 17 — Acceptance Report

|              |                                       |
| ------------ | ------------------------------------- |
| **Phase**    | 17 — The Owner's Control Over Workers |
| **Branch**   | `claude/phase-17` (TBD PR link)       |
| **Verified** | TBD                                   |
| **Date**     | 2026-09-27 (Pacific time)             |
| **Result**   | TBD                                   |

Screenshots (from the end-to-end run in the real app, `tests/e2e/specs/control.e2e.mjs`):

- **Settings → AI models:** [Model and effort rules](evidence/phase-17/control-rules-settings.png)
  (the organization at high effort, and every role's reason naming the rule)
- **An agent's details, tab by tab:** [Overview](evidence/phase-17/control-panel-overview.png) ·
  [Job](evidence/phase-17/control-panel-job.png) (the Database specialty, learning off for this
  agent) · [AI model](evidence/phase-17/control-panel-ai-model.png) (max effort, "from this
  agent's own setting") · [Work](evidence/phase-17/control-panel-work.png) (its permissions) ·
  [Team](evidence/phase-17/control-panel-team.png) ·
  [Manage, widened](evidence/phase-17/control-panel-manage-wide.png)
- **Archive and delete for good:** [the Archived list](evidence/phase-17/control-archived-list.png) ·
  [Delete for good, offering the Workforce](evidence/phase-17/control-delete-for-good.png) ·
  [a whole department archived](evidence/phase-17/control-archived-department.png)
- **The Workforce:** [an agent saved, ready to hire again](evidence/phase-17/control-workforce.png)

TBD: test totals.

On screen the plan's words become plain ones ([word list](../design/vocabulary.md)): a "layer"
is a **rule** ("the Development department's rule"), a "position" is an **agent** on the chart,
"restore" is **Bring back**, "hard delete" is **Delete for good**, a "tombstone" is a **short
record**, a "brief" is the **full instructions** or a **short reminder**, "compaction" is
**shortened its memory**, and the owner's "score" is **Experience**. Quotes from the plan keep
the plan's words.

CI has no AI tool accounts. The end-to-end tests use Plenipo's stand-in AI tool
(`plenipo-fake-agent`), which reports the effort it was given, and a throwaway home folder.

## 1. Acceptance criteria → evidence

TBD

## 2. Deliverables → evidence

TBD

## 3. Plan tests → evidence

TBD

## 4. The owner's decisions and rules → evidence

TBD

## 5. Prompts sized to the job: measured before and after

TBD

## 6. Deviations from the plan

- **Authorized penetration testing is not a built-in Security Auditor specialty.** The plan lists
  it; Plenipo ships 20 built-in specialties, not 21. You can add it yourself as one of your own
  specialties, with your own lines, on the Security Auditor role (or any role); Guard still
  decides what that worker may do, and asks you before each program and each server as your
  permission sets say. Recorded in ADR-042 (specialties), "As built".
- **Numbers for the records:** ADR-041 to ADR-045. The open pull request with the security fixes
  (Group B) numbers its records ADR-036 to ADR-038, which `main` already uses; it will need new
  numbers when it merges.

## 7. Defects found and fixed during Phase 17

TBD

## 8. Left for the owner (on Windows)

TBD
