# ADR-196: Use the team you hired first, and ask before hiring a missing worker

- **Status:** Accepted (the owner, 2026-10-03: "It created new senior devs and code reviewers
  instead of using the ones I set up … we need to use the agents already available. If it's
  missing an agent the manager needs, then it can spawn that agent"; item 2.7 of
  [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md), with the owner's answer 3: ask first,
  with a switch to let it hire on its own).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 2 (item 2.7)
- **Amends:** [ADR-016 (the Development department)](ADR-016-development-department.md): a new
  project's team is not always hired fresh. [ADR-054 (move or lend an agent)](ADR-054-move-or-lend.md):
  a lead can hand work to its department's on-call workers without a loan.

> **On screen** (ADR-010, plain words and rank names): the "Set up a Development project" dialog
> asks for the **Department** and shows each job as "Use Senior Developer (Senior Developer,
> Opus (Claude Code))" or "Hire new". Home's What's stuck says "Website Supervisor needs a
> Security Auditor" and "Hire one?". Settings → Switches has "Let leads hire missing workers on
> their own".

## In short

Setting up a project always hired a fresh team, even when the department already had the same
workers. And a supervisor could only hand work to the workers reporting to it, so the workers you
had hired under another supervisor were out of its reach.

**Accepting this record means:**

1. **No copies at setup.** Each job of a new project's team uses a matching on-call worker the
   department already has. Only the missing jobs are hired, unless you choose "Hire new". You pick
   the department; a new Development department is made only when you ask for one.
2. **The department shares its workers.** A lead hands work to its own team first, then to its
   department's other on-call workers. The work is the asking project's: its folder, its allowed
   AI tools, and its department's limits. A worker keeps the AI tool and model you chose for it.
   Full-time workers outside the team are still reached by lending them (ADR-054).
3. **Ask before hiring a missing worker.** When nobody in the department does a job, Plenipo asks
   you on Home ("Hire one?"), once a day per lead and job, and tells the lead to do that part itself
   meanwhile. Choosing it hires one on call for that lead's team. The question goes away once the
   team or department has someone for the job.
4. **Or let it hire on its own.** With "Let leads hire missing workers on their own" on (it starts
   off), Plenipo hires the on-call worker straight away and hands it the work.
5. **Leads are told:** "Use the people you have first."

## Consequences

- Setting up a second project in the same department hires nobody new by default.
- A worker can be busy with two projects' work at once; on-call workers each start a fresh worker
  per task, so nothing waits that didn't before.
- A lead's team list now includes its department's other workers, marked "from your department,
  when your team has no one for it".

## Alternatives considered

- **Always hire on its own.** Rejected by the owner (answer 3): ask first.
- **Lend workers automatically.** Rejected: a loan moves the worker's whole day to another team,
  and on-call workers don't need one.
