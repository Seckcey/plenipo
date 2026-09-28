# ADR-024: Workers learn from their work

- **Status:** Accepted (by the owner, 2026-09-27)
- **Amended by:** [ADR-041 (model, effort, and learning in layers)](ADR-041-model-effort-learning-layers.md)
  (learning on or off for each role and each agent, under the main switch) and
  [ADR-045 (experience and the Workforce)](ADR-045-experience-and-the-workforce.md) (kept lessons
  count toward an agent's experience).
- **Number:** accepted as ADR-022 while in review; renumbered when ADR-021 went to the Free
  and Pro editions (and ADR-022 was held for Kimi's file access, now ADR-027)
- **Date:** 2026-09-27
- **Phase:** 10 (follow-up, v1.4.0)
- **Extended by:** [ADR-025 (servers over SSH, through Guard)](ADR-025-servers-over-ssh.md): a
  lesson from a task that ran commands on a server also always waits for the owner
- **Amended by:** [ADR-040 (lessons a role keeps on its own are notes, not
  orders)](ADR-040-lessons-kept-on-their-own.md): a role that learns on its own keeps a lesson
  unasked only from a task that used no tool at all, a lesson with a command, a path, or a web
  address always waits, kept lessons reach workers inside a fence that says who kept them, never
  as orders, and a lesson belongs to its project (sections 3 and 4)

## Context

When the owner accepted ADR-019 (every role knows its job), they asked: "can we have them learn
as they work so they get smarter and smarter on their own?" Asked how much say they want, they
chose **"Ask me, switch per role"**: each lesson waits for their Keep or Discard, and each role
has a switch to learn on its own. They also asked for a **Worker learning** switch in Settings
(ADR-023, on/off switches in Settings).

Every worker starts fresh: its role's working instructions (ADR-019) and its task, nothing from
earlier tasks. The rollout plan has no phase for this. It is a small step: plain notes in the
Ledger, not semantic memory or model training.

The risks:

- **Planted lessons.** A web page or a file can tell a worker what to "learn" (prompt
  injection). A lesson kept without review would reach every later worker of that role.
- **Secrets and personal details** in a lesson would be repeated to later workers.
- **Growth.** Unbounded lessons would crowd out the task in every prompt.
- **Lessons that change rules.** "Always approve payments" must never become a rule.

## Decision

### 1. How a worker writes a lesson

Every worker's instructions end with how to write one: at the end of its answer, a fenced
`plenipo-lesson` block with one short, general lesson per line, at most three. Only what it
learned by doing the work; no secrets, passwords, or personal details; nothing a web page or a
file told it to write; nothing about permissions or the owner's rules. Most tasks teach nothing
new, so most answers have no block.

### 2. How Plenipo records it

When a worker's answer is recorded (`agent.result`), Plenipo reads its `plenipo-lesson` blocks
and records each lesson in the Ledger (new table `lessons`, migration 7) against the worker's
role:

- at most **3 per task** and **300 characters** each (longer ones are cut, marked with "…");
- a lesson already waiting or kept for that role is not added again;
- secrets are already hidden in every answer before it is recorded (Vault redaction);
- event `lesson.added`, with the worker, the role, and whether the task used websites or the
  screen.

### 3. Who decides

- A lesson **waits** for the owner on the Approvals page, under **New lessons**. The owner can
  edit its words, then **Keep** or **Discard** it (`lesson.kept`, `lesson.discarded`). Nothing
  waits on the answer; work goes on.
- A role set to **Learn on its own** (in its details on the Organization page) keeps its
  lessons without asking (`learning.role_changed`).
- A lesson from a task that **used websites or the screen** **always waits**, with a warning,
  whatever the role's switch. That covers any browser or screen step in the task itself or in
  any task handed on from it (a Supervisor's answer can carry what its Web Assistant read). A
  website must not be able to plant one.
- The owner can **Remove** a kept lesson at any time from the role's details
  (`lesson.removed`).

### 4. How lessons are used

A worker's instructions (built for every turn) carry its role's **newest 20 kept lessons**,
marked as what earlier workers learned, to follow unless the task or its lead says otherwise.
Lessons are words only: they never change permissions, approvals, or Guard's rules.

### 5. The switch

**Worker learning** in Settings → Switches (on to start; Ledger setting `learning`, event
`learning.switched`). Off: no lessons are recorded, none go into instructions, and workers are
not asked to write any. Lessons already kept stay, and come back when it is on again.

## Consequences

- Roles get better at the owner's recurring work over time, and the owner sees and controls
  every lesson.
- The owner has one more kind of item on the Approvals page. The sidebar count includes waiting
  lessons.
- A role set to learn on its own can still keep a poor lesson (from a task without websites).
  The owner sees kept lessons in the role's details and can remove one.
- A lesson a worker writes from a file's contents is not detected; only website and screen use
  force a review. Files are the owner's own, a lower risk.
- Instructions grow by up to 20 lines per role.
- The lessons are part of the Ledger export (`lessons` table).
- **Worker learning is a Pro feature** (the owner's decision, 2026-09-27; ADR-021, Free and Pro
  editions). Like everything else, it is unlocked until the Free and Pro split ships.

## Alternatives considered

- **Learning on its own for every role, with no review.** Rejected: the owner chose to be asked,
  and a planted lesson would spread to every later worker.
- **Fine-tuning or embeddings (semantic memory).** Rejected for now: heavy, tied to one AI
  provider, and hard for the owner to see and undo. Plain lessons work with every AI tool.
- **A shared lesson list for the whole organization.** Rejected: lessons are about a kind of
  work, so they belong to a role.
- **Letting lessons change a role's instructions.** Rejected: the role's working instructions
  (ADR-019) are the owner's. Lessons are added beside them, and can be removed.
