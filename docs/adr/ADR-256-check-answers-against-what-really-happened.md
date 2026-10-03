# ADR-256: Check answers against what really happened

- **Status:** Accepted (the owner, first list, kept in Phase 25: "Supervisors and up detect
  hallucinations and act accordingly."; item 4.7 of
  [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md)).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 4 (item 4.7; item 4.8 is step 2)
- **Number:** in the 250s with Phase 25's other later decisions (see ADR-250).
- **Builds on:** [ADR-008 (Liaison: how workers hand work to each other)](ADR-008-liaison.md)
  and [ADR-016 (the development department and an objective's result)](ADR-016-development-department.md).

> **On screen** (ADR-010, plain words and rank names): a reply in a worker's handoffs says
> "**Doesn't match the record**", and under it "Doesn't match Plenipo's record: says tests
> passed, but no test ran. It was sent back to the worker once to check." The Ledger says
> "Answer sent back to check: it says tests passed, but no test ran" and "Checking its answer
> again".

## In short

Before, when a worker finished, its own words went up to its lead and nothing else. Plenipo
already kept what really happened (the files changed, the programs and tests run with pass or
fail, the pull requests opened), but only you saw that, in the objective's result. A lead read
only the worker's story.

**Accepting this record means:**

1. **Every answer handed back up the chain carries Plenipo's record** under the worker's words:
   files changed, how many programs ran, the tests and checks run (passed or failed), and pull
   requests opened.
2. **Four plain checks run first:**
   - "says tests passed, but no test ran"
   - "names a file it didn't change: src/app.ts"
   - "says it opened a pull request, but none was opened"
   - "a review with no verdict" (only for workers asked for one: reviewers, QA, security
     auditors, and anyone serving a team through oversight)
3. **When a check fails, the answer goes back to its worker once**, with the reasons and the
   record: "Check your work and answer again … an honest 'not done' is better than a wrong
   'done'." If its next answer still fails, it goes to its lead marked "doesn't match the
   record". It is never sent back twice.
4. **Leads are told to compare** each reply's words with the record before passing work up, and
   not to repeat a claim the record doesn't back.

## Decision

- **Where the record comes from.** Only Plenipo's own Ledger (ADR-060's rule: only what was
  recorded). For the worker's task and every task handed on from it: Plenipo's tools
  (`capability.used`: commands with pass or fail, file changes, files made by commands, pull
  requests), each AI tool's own steps (`agent.tool_use` and `agent.tool_result`: Claude Code's
  Bash, Edit, and Write; Codex's commands and file changes; the same for every AI tool that reports
  its steps), and the files on the objective's working copies (git).
- **What counts as a test.** The same reading as an objective's result (Phase 8): a command with
  a word like `test`, `pytest`, `jest`, `check`, `verify`, or `e2e`.
- **The checks lean towards trusting the worker.** A sentence with "not", "didn't", "will",
  "should", "if", "once", "need", or "failed" is never read as a claim. A file counts as touched
  when any step on the record names it (a command that wrote it counts). A pull request counts
  when any step names one. At most three files are named in one check. A wrong "doesn't match"
  costs the worker one more look; a missed one is what the record under the answer is for.
- **Which answers.** A handoff's final answer (one that hands nothing on): workers and full-time
  members alike. Your own objective's answer to you is not sent back; you already see the
  objective's result.
- **How it is sent back.** In the worker's own conversation, as the next step of the same task
  (the way replies are given): the task waits (`liaison.answer_sent_back`, with the reasons and
  the record), and Liaison gives it back (`liaison.sent_back_delivered`). Free's three workers at
  a time apply (ADR-113). If the worker's conversation is gone, the task ends and its answer goes
  up as it was, marked.
- **Recorded.** The reply keeps the record, the mismatches, and whether it was sent back
  (`record`, `mismatches`, `sentBack`); the Ledger's reply events say "doesn't match the record".

## Consequences

- **Easier:** a lead, and you, see at once when a worker's words and its work disagree, and a
  worker gets one chance to fix it before anyone else sees it.
- **Harder:** an answer that fails a check costs one more step on the worker's AI tool.
- **Watch for:** an AI tool that does work without reporting its steps. Its true "tests passed"
  would fail the first check. Every AI tool Plenipo runs today reports its steps; one that
  doesn't must be added to the record before it is trusted with this.

## Alternatives considered

- **Have an AI model judge every answer.** Rejected for step 1: it costs a call per answer and
  can be wrong in ways nobody can check. Plain checks are free and say exactly why.
- **Send an answer back until it matches.** Rejected: a worker that can't do the work would loop.
  Once, then up to the lead, marked (the plan's rule).
- **Hide the record from leads unless a check fails.** Rejected: the record is short, and a lead
  that compares catches what the plain checks miss.
- **Step 2 (item 4.8):** checking that links and pull requests named in an answer really exist,
  supervisors stopping a worker or sending work back themselves, and a notice when a worker's
  answers keep failing.
