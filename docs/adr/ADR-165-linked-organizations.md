# ADR-165: Linked organizations — linking, sending an objective, the answer, and unlinking

- **Status:** Accepted (2026-10-02): the owner answered questions 6 and 11 in
  [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md) "as recommended".
- **Date:** 2026-10-02
- **Phase:** 24 (Community)
- **Part of:** [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md)
- **Builds on:** [ADR-145 (the fixed list of what a phone may ask)](ADR-145-what-a-phone-may-ask.md),
  whose shape this follows: a fixed list, Guard first, the same core function as the PC;
  [ADR-164 (private messages, sealed)](ADR-164-private-messages-sealed.md) for how things travel
- **Waited for:** pull requests #136 (Guard on a Mac and Linux) and #138 (the keeper), before any
  of Guard's code changes ([ADR-160](ADR-160-phase-24-alongside-phase-23.md) §4). Both merged on
  2026-10-02.

> **On screen** (ADR-010, plain words and rank names): **Linked organizations**, **Link with
> another organization**, **Pat Lee wants to link Acme Builders with your organization**,
> **Accept** / **Not now** / **Block**, **Objectives from Acme Builders go to: VP**, **Send an
> objective to Acme Builders**, **An objective from Acme Builders (Pat Lee)**, **Approve** /
> **Refuse**, **Waiting for their approval**, **Send the answer back to Acme Builders?**,
> **Don't accept objectives for now**, **Unlink**.

## In short

Two owners can **link** two organizations, when both agree. Then either can **send the other an
objective** in words. It waits in the other owner's **Approvals**, and runs only if they approve it,
under **their** Guard, **their** permissions, and **their** AI tools, like any objective they give
themselves. Their workers treat the words as **outside words**, like an email. When the work is
done, the receiving owner **reads the answer and chooses** whether to send it back. Nothing else
crosses: no files, no folders, no sign-ins, no keys, no Ledger. Either owner can **Unlink** at any
time, at once.

## Context

- Phase 24's plan: "two owners agree to link. One organization can send an objective to the other,
  and the other owner's Guard and approvals decide it, like a request from the web interface
  (Phase 14)". Tests: the objective "waits for the receiving owner's approval and runs under their
  Guard"; a blocked person cannot send objectives; nothing crosses unless an owner sends it on
  purpose.
- A phone's objective is already text only, checked by Guard against a fixed list, and handed to the
  PC's own core function (`Workforce::give_objective`) (ADR-145).
- The other organization's PC may be off, so an objective has to wait somewhere; sealed messages
  already do (ADR-164).
- Words from another business are the clearest case of untrusted input: they could try to talk a
  worker into leaking files or running something (plan §3.1).

## Decision

1. **Linking** (question 6):
   - Owner A picks one of their organizations and **Link with another organization**, types the
     other owner's Community name, and adds a short note. Both sides need Pro (ADR-162 §5).
   - Owner B sees "**Pat Lee wants to link Acme Builders with your organization**" with **Accept**,
     **Not now**, and **Block**. Accepting asks which of B's organizations, and which position
     receives objectives (B's **VP** to begin with; any position that takes objectives). A asks the
     same on their side when B accepts.
   - Both see a **safety code** for the link (as ADR-164 §2), and **What crosses this link**: "Only
     objectives in words, the answers you choose to send back, and whether each objective was
     approved, refused, stopped, or finished."
   - A link joins **one organization to one organization**. An owner with three organizations
     links each one separately.
   - A request not answered in 14 days lapses.
2. **Sending an objective:** **Send an objective to Acme Builders**, in words only, up to the same
   size limit as an objective on the PC. No files, no attachments, no folder or file paths, no
   choice of worker, AI tool, model, or permission. It is sealed for the other organization's PCs
   and travels like a message (ADR-164). Limits: at most 5 waiting at once on a link, and 20 a day.
3. **Receiving one:** it arrives as an approval, "**An objective from Acme Builders (Pat Lee)**",
   showing the words exactly, the link, and the position it will go to.
   - **Approve** gives it to that position through the same core function the PC uses
     (`Workforce::give_objective`), with all of its rules. **Refuse** ends it.
   - It is marked as **outside words** for every worker that sees it (like email in Phase 20): the
     workers treat it as a request from someone else, and Guard decides everything they then do, as
     always. Approving the objective approves **nothing else**: every later approval still asks.
   - It shows on your phone like any approval (ADR-145), and **Keep these approvals on my PC only**
     gains a box, "**Objectives from linked organizations**", not ticked to begin with.
4. **Guard decides first**, in Guard's request path (the part ADR-145 built for phones), in this
   order: Community is on and signed in; the link is active and not paused; the sender is not
   blocked; both sides are Pro; the request is on the fixed list (a linked organization's list has
   one kind: an objective); its size and the link's limits. Anything else ends the request, runs
   nothing, and is recorded as refused, with one plain sentence.
5. **What goes back** (question 11):
   - **Automatically:** only the state of each objective: **approved**, **refused**, **stopped**,
     or **finished**. No words with it.
   - **On purpose:** when the work is finished, the receiving owner sees "**Send the answer back to
     Acme Builders?**" with the final answer's words, which they can change, and **Send** or
     **Don't send**. Only those words go, sealed. Never the task's conversation, files, file paths,
     Activity, or anything from the Ledger.
6. **Pausing and unlinking:**
   - **Don't accept objectives for now** pauses a link from your side; new ones are refused with
     "Not accepting objectives right now".
   - **Unlink** works at once, from either side, with no agreement needed. Objectives still waiting
     are refused. Work already approved keeps running in the receiving organization as its own work
     (its owner can stop it), but no answer can be sent back. Linking again needs a new request.
   - **Blocking** the other owner unlinks every link with them (ADR-167).
   - When Pro ends on either side, the link pauses; nothing is deleted.
7. **Recorded in both Ledgers,** each side its own part: `link.requested`, `link.accepted`,
   `link.paused`, `link.ended` (with who and why), `link.objective_sent`,
   `link.objective_received`, `link.objective_refused` (with Guard's or the owner's reason),
   `link.answer_sent`, `link.answer_received`. An approved objective is recorded as the PC records
   objectives today, with **from Acme Builders** on it. The `source` of the owner's approval stays
   `owner`; the objective's origin is named in the event.
8. **New desktop commands are the main window's alone**, with IPC tests that refuse a second window,
   the sign, and a web page: link, accept, pause, unlink, send an objective, and send the answer.

## Consequences

- Two businesses can hand each other work without either reaching into the other's PC.
- The receiving owner reads every objective before it runs. That is slower than letting a link run
  work by itself, on purpose.
- A worker can still be fooled by clever words. Approving the objective is the first gate; Guard and
  every later approval are the rest, exactly as for email today.

## Alternatives considered

- **Objectives run without approval from a trusted link.** Faster, but the plan says the receiving
  owner's approval decides it, and words from outside are untrusted.
- **Send the whole result back automatically** (the conversation, changed files). That is Ledger and
  file content crossing without the owner choosing it; out.
- **Through the relay, live.** Both PCs would have to be on at the same moment.
- **Let the sender choose the worker or the AI tool.** That reaches into the other organization's
  setup; the receiving owner decides.
