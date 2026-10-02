# ADR-145: The fixed list of what a phone may ask, and what stays on your PC only

- **Status:** Accepted (by the owner, 2026-10-01, as recommended): none of the approvals is kept on the PC to begin
  with (question 7). The face, fingerprint, or passcode check is asked only at sign-in (ADR-142), so
  every request on the list needs a signed-in phone.
- **Date:** 2026-10-01
- **Phase:** 14
- **Part of:** [ADR-140 (Phase 14 starts)](ADR-140-phase-14-starts.md)
- **Carries out:** the plan's "The connection offers a fixed list of requests ..., each checked by
  Guard. It does not reuse the desktop window's commands", and ADR-040 §4 (what never reaches the
  web interface)

> **On screen** (ADR-010, plain words and rank names): **Use Plenipo from another device** (the
> switch in Settings → Switches), **Part of Pro**, **Settings → Devices**, **Approve on your PC**,
> **Keep these approvals on my PC only**, **from Frank's iPhone** (in Activity), **Coming soon**.

## In short

A phone can ask your PC for a **short, fixed list of things**, and nothing else. It can read every
page, answer approvals, stop work, start it again, and give a Manager an objective. It can never
reach the terminal, a shell, files, the screen, Plenipo's browser, or secrets, and it can never
change what workers may do or who may connect. Guard checks every request, and the Ledger records
each one with the phone that sent it. You can mark kinds of approvals **Approve on your PC**; to
begin with, there are none, as the plan says.

## Context

- The plan's list: every page to read; approve, refuse, and allow; keep or discard a lesson; send an
  objective to a manager; stop a task, Stop all, Allow again, Run again, and Leave stopped; and the
  web interface's own choices. What stays on the PC: the terminal and any shell, files, the screen
  and Plenipo's browser, secrets, and anything that widens what workers may do or who may connect.
- Each of these already has one core function on the PC (ADR-140, Context 2).
- Today an objective can bring files along. Files are the PC's alone, so a phone's objective is text
  only.
- The Ledger's `source` says who acted, and several checks look for `owner` (ADR-140, Context 8).

## Decision

1. **The switch:** Settings → Switches → **Use Plenipo from another device**, off to begin with
   (main window only). On Free it says **Part of Pro**, with what Pro adds, and cannot be turned on.
   Turning it off cuts every phone off at once (ADR-143 §7). A new limit, `Limit::PhoneAccess`, is
   the one place that decides it (`Entitlements::check`).
2. **The list is a fixed set of request kinds in code** (one Rust `enum`, with no "any command"
   kind). Anything not on it cannot even be written down, and Guard refuses a request it cannot read.

   | What the phone asks                                                                                                                                                                                                 | Needs                            | Part |
   | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------- | ---- |
   | **Read:** Home; Approvals; the organization; projects; workers; tasks and their conversations; Activity; AI tools; Diagnostics; whether everything is stopped; lessons waiting; the list of organizations on the PC | Signed in                        | 14A  |
   | **Refuse** an approval                                                                                                                                                                                              | Signed in                        | 14A  |
   | **Approve** an approval                                                                                                                                                                                             | Signed in                        | 14A  |
   | **Stop all**                                                                                                                                                                                                        | Signed in                        | 14A  |
   | **Sign out**; **Remove this phone**                                                                                                                                                                                 | Signed in                        | 14A  |
   | **Allow again** after Stop all                                                                                                                                                                                      | Signed in                        | 14B  |
   | **Stop** one worker's task                                                                                                                                                                                          | Signed in                        | 14B  |
   | **Run again** / **Leave stopped** after an unexpected stop                                                                                                                                                          | Signed in                        | 14B  |
   | **Keep** / **Discard** a lesson, as written                                                                                                                                                                         | Signed in                        | 14B  |
   | **Send an objective** to a Manager, Supervisor, or other position that takes objectives (text only, up to the PC's own size limit)                                                                                  | Signed in                        | 14B  |
   | **Refuse** / **Discard** right from a notice                                                                                                                                                                        | The phone's own key (ADR-142 §5) | 14C  |

   The phone's own choices (its notices, the lock-screen words, its theme) live on the phone and ask
   the PC nothing.

3. **What stays on your PC only**, never on the list:
   - the terminal, any shell, and running a program;
   - files: opening, saving, attaching them to an objective, **Show in folder**;
   - the screen, the mouse and keyboard, **Take over**, and Plenipo's browser;
   - secrets and the Vault;
   - AI tools: signing in or out, updating, and starting an AI tool conversation directly;
   - permissions, permission sets, approved programs, the never-run lists, Guard's rules, the
     switches, and spending caps;
   - Connections and add-on tools;
   - the license;
   - organizations (new, copy, archive, delete) and the workforce (bringing in, moving, archiving, or
     deleting workers; their AI models and effort);
   - **Settings → Devices** (adding, renaming, un-pausing) and turning phone access on;
   - **Keep these approvals on my PC only** (item 5);
   - installing updates, and saving a diagnostics file.
4. **Guard decides every request**, in a new part of Guard (`guard::remote`), in this order: phone
   access is on; Pro; the phone is paired and not paused; the meeting is signed in and not ended
   (except **Refuse** and **Discard** sent from a notice, which need only the phone's own key,
   ADR-142 §5); the request is on the list; an approval is not one kept on the PC (item 5). Each refusal has one plain sentence, as Guard's other
   refusals do. Then the PC calls the same core function the main window uses, which applies all of
   its own rules too (an approval answered once, expired, or gone; Free's limits; a project that is
   not active).
5. **Approvals kept on your PC:** Settings → Devices → **Keep these approvals on my PC only**, a list
   of check boxes, **none ticked to begin with** (the plan): each kind of sensitive action Guard knows
   (ADR-013 §8: production changes, DNS, credentials, databases, cloud deletions, payments, messages
   and publishing, running as administrator, changes outside the project folder), **commands on a
   Production server**, and **every approval**. A kept approval shows on the phone, with what it is,
   and "**Approve on your PC**", with no buttons; the PC refuses an answer to it from a phone even if
   one is sent.
6. **The Ledger records every request with the phone that sent it.**
   - Each request becomes `remote.request` (what kind, the target's ID, and the phone's ID and name)
     or `remote.refused` (the same, and why). Reading is recorded once for each page the phone opens,
     not for each refresh.
   - What the request does is recorded as it is from the PC (for example `approval.resolved`), with
     `source` still `owner`, and the phone named in the event: "**Approved by you, from Frank's
     iPhone.**" So Activity shows it, and every check that looks for the owner still works.
   - Objectives' text is recorded as the PC records it today. Nothing else from the phone is.
7. **More than one organization** (Phase 21): phones belong to the PC, like AI tool sign-ins. A phone
   sees every organization on the PC, chooses which one it is looking at, and sees approvals from all
   of them, each with its organization's name. **Stop all** and **Allow again** act on the whole PC, as
   the tray's do. Each request is recorded in the Ledger of the organization it touches.
8. **New desktop commands are the main window's alone**, in `capabilities/default.json`, with tests
   that a second window, the sign, and a web page are refused: the switch, **Add a phone**, the
   pairing answer (**Add** / **Cancel**), **Rename**, **Remove**, un-pause, the list of devices, and
   **Keep these approvals on my PC only**.
9. **Until the relay is live** (ADR-140 §4), a released copy shows the switch as **Coming soon**.

## Consequences

- A stolen phone, even unlocked and signed in, cannot widen anything: the list has no way to change
  permissions, add a phone, or reach a file.
- Adding a request later is a new kind in the list, a Guard rule, a test, and a line in this record.
- Approving from a phone and from the PC run the same code, so they cannot drift apart.

## Alternatives considered

- **Reuse the desktop window's commands through the relay.** The plan forbids it: the desktop has
  hundreds of commands, and some must never leave the PC.
- **Some approvals kept on the PC from the start** (payments, credentials, running as administrator,
  Production servers). Safer by default, but the plan says none to begin with; you can tick them in
  one place.
- **Files on an objective from the phone.** Files are the PC's alone (the plan). A phone could send
  text pasted into the objective instead.
- **A new `source` for phones** (for example `phone:…`). The Ledger would be more exact, but every
  check that looks for the owner would need to change, and the owner is still the one acting.

## As built (v1.19.0, part 14A)

Built as decided. The fixed list is one `enum`, `guard::remote::RequestKind`, and Guard's checks run
in the order of §4 (`guard::remote::decide`). Anything not on the list, or with a field the list
does not have, ends the meeting at once and runs nothing (a test asks for a program, a file, the
terminal, and a switch). Every request is recorded with the phone that sent it: `remote.request`
(the phone's ID and name, the kind, and its target) or `remote.refused` (with Guard's reason); a page
read again after a change is not recorded twice. An approval answered from a phone carries "Approved
by you, from Frank's phone.", and Activity says "Frank's phone asked to approve" and "Approved: git
push origin (from Frank's phone)". **Stop all** from a phone is the PC's own Stop all (browser,
desktop, and server work, in every organization), and the phone's words say so. The PC already
carries out part 14B's requests; the page offers them in 14B.
