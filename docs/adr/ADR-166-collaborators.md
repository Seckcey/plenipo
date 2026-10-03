# ADR-166: Collaborators — viewer, approver, and manager, and what each may do

- **Status:** Accepted (2026-10-02): the owner answered questions 5 and 10 in
  [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md) "as recommended".
- **Date:** 2026-10-02
- **Phase:** 24 (Community)
- **Part of:** [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md)
- **Builds on:** [ADR-143 (the relay and the lock)](ADR-143-the-relay-and-the-lock.md) and
  [ADR-145 (the fixed list of what a phone may ask)](ADR-145-what-a-phone-may-ask.md): a
  collaborator reaches your PC the way your phone does, with a shorter list
- **Changes:** [ADR-145](ADR-145-what-a-phone-may-ask.md) §6 keeps `source` as `owner` for your
  phone, because the phone is you. A collaborator is **not** you, so their actions get a source of
  their own (Decision 6).
- **Waited for:** pull requests #136 and #138 before any of Guard's code changes
  ([ADR-160](ADR-160-phase-24-alongside-phase-23.md) §4). Both merged on 2026-10-02.

> **On screen** (ADR-010, plain words and rank names): **Collaborators**, **Invite someone to help
> with this organization**, **Viewer**, **Approver**, **Manager**, **What they can see**: **The whole
> organization** / **Only these departments and projects**, **Approvals only you can answer**,
> **Pat Lee invited you to help with Frank's organization as an Approver**, **Shared with you**,
> **Approved by Pat Lee (Approver)**, **Remove**, **Leave**.

## In short

You can invite a person to help with one of your organizations, as a **Viewer** (looks), an
**Approver** (looks, and answers approvals), or a **Manager** (also gives objectives and brings in
workers). They use **their own Plenipo**, on their own PC, and reach yours through the same sealed
line your phone uses. **You stay on top:** they never touch the terminal, files, the screen, keys,
permissions, or settings; some approvals stay **yours only**; every step they take is in your
Ledger with their name; and **Remove** ends their access at once.

## Context

- Phase 24's plan: "invite a person into your organization as a viewer, an approver, or a manager.
  Every action they take is recorded, and the owner stays on top". Tests: "a viewer cannot approve;
  an approver cannot hire"; "removing a collaborator ends their access at once". Acceptance: a
  collaborator "approves a task from their own Plenipo".
- Your phone already reaches your PC through Plenipo's relay, sealed end to end (Noise), with a
  fixed list of requests that Guard checks (`guard::remote::RequestKind`, 26 kinds today), and the
  PC's own core functions doing the work (ADR-145).
- Today the only person Plenipo knows is you: Guard's checks and the Ledger look for
  `source = "owner"`, and nothing models a second person (the check in ADR-161).
- The relay checks that your PC is Pro, and passes messages only with a pass your PC signed
  (ADR-143 §3, §4).

## Decision

1. **The roles** (question 5). Each role adds to the one before it:

   | Role         | What they may do                                                                                                                                                                                                                                                                                                                                                                                                                    |
   | ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
   | **Viewer**   | Read, in the part they may see: Home, Approvals (to see, not answer), the organization chart, projects, workers, tasks and their conversations, Activity, and lessons waiting                                                                                                                                                                                                                                                       |
   | **Approver** | Everything a Viewer may, plus **Approve** and **Refuse** approvals (except those only you can answer, item 3), and **Stop** one worker's task                                                                                                                                                                                                                                                                                       |
   | **Manager**  | Everything an Approver may, plus **send an objective** (words only) to a position in their part; **Run again** / **Leave stopped**; **Keep** / **Discard** a lesson as written; and **bring in** a worker into a position you already made, from your Workforce or on call, with the AI tool and model you already set for that role. Never a new role, department, or project, and never a change to a worker's AI model or effort |

2. **Never, for any collaborator**, whatever the role: the terminal and any shell, running a
   program, files (open, save, attach, **Show in folder**), the screen, mouse and keyboard, **Take
   over**, Plenipo's browser, secrets and the Vault, AI tool sign-ins and updates, permissions,
   permission sets, approved programs, the never-run lists, Guard's rules, the switches, spending
   caps, Connections and add-on tools, the license, organizations, archiving or deleting anything,
   Settings → Devices, collaborators and links, Community's settings, **Stop all** and **Allow
   again** (they act on your whole PC), the AI tools page, Diagnostics, your Workforce page, and your
   other organizations. This is the phone's "stays on your PC" list (ADR-145 §3), and more.
3. **Approvals only you can answer** (question 10). Each invitation has this list of boxes, **all
   ticked to begin with**: each kind of sensitive action Guard knows (ADR-013 §8: production
   changes, DNS, credentials, databases, cloud deletions, payments, messages and publishing, running
   as administrator, changes outside the project folder), **commands on a Production server**, and
   **objectives from linked organizations** (ADR-165). A collaborator sees such an approval with
   "**Only Frank can answer this**" and no buttons; your PC refuses their answer even if one is sent.
   You can untick boxes for one person, later, on your PC. (Your phone is different: it is you, so
   none is kept from it to begin with, ADR-145 §5.)
4. **What they can see** (question 10): every invitation must choose **The whole organization** or
   **Only these departments and projects**; nothing is chosen for you. A collaborator never sees
   another organization on your PC, and a task's conversation shows only for the part they may see.
5. **How they connect:**
   - **Inviting:** you pick the organization, type their Community name or email (ADR-163), the role,
     the part, and the approvals only you can answer. The invitation travels sealed (ADR-164) and
     lapses after 7 days. Inviting needs Pro; accepting needs only a free 8 West account
     (ADR-162 §5).
   - **Accepting** in their Plenipo swaps public keys between their PC and yours (each PC's own
     Community key, ADR-162 §3), with a **safety code** to compare (ADR-164 §2). Your PC then signs
     a **collaborator pass** for each of their PCs, like a phone's relay pass (ADR-143 §4).
   - **Every meeting** is the phone's lock: `Noise_KK_25519_AESGCM_SHA256` through Plenipo's relay,
     with the same counters, request IDs, and wrong-try limits (ADR-143 §5 to §8). Your PC must be on
     for them to see or do anything; nothing waits at the relay.
   - The relay's contract gains the collaborator pass (`contracts/phone-relay/v2`), and the stand-in
     relay's **bad relay** mode tests it, as in Phase 14.
6. **Guard decides every request**, in Guard's request path, extended with **who is asking**: you
   (your phone), or a collaborator with a role and a part. The order: Community and collaborators
   are on; your PC is Pro; the collaborator is not removed, paused, or blocked; their pass and
   meeting are valid; the request is on **their role's list** (each role's list is a fixed set in
   code, like the phone's); the target is in their part; and an approval is not one only you can
   answer. Then the PC calls the same core function the main window uses, with all of its rules.
7. **Recorded as them, never as you.**
   - The Ledger's `source` for a collaborator's action is `collaborator:<their ID>`, never `owner`,
     and the event names them and their role: "**Approved by Pat Lee (Approver).**" Every check that
     looks for the owner therefore refuses a collaborator by itself.
   - Their requests are recorded as `collab.request` and `collab.refused` (with Guard's reason), as
     the phone's are; reading is recorded once for each page, not each refresh.
   - Their PC keeps its own record of what they did (in their Ledger, under **Shared with you**).
8. **Their words are outside words.** An objective a Manager collaborator sends reaches workers
   marked as from another person (like email in Phase 20, and like ADR-165 §3).
9. **Removing and leaving:**
   - **Remove**, on your PC, ends it **at once**: your PC ends their meetings, forgets their keys,
     tells the relay to drop their passes, and tells the account service. Anything they sent that is
     still in flight is refused. Recorded as `collab.removed`.
   - **Leave**, on theirs, does the same from their side.
   - **Pause** keeps them listed but lets nothing through.
   - When Pro ends on your PC, collaborators pause, like phones (ADR-143 §7). Nothing is deleted.
   - Blocking them removes them (ADR-167).
10. **Limits:** at most 10 collaborators for each organization, each with at most 5 PCs.
11. **On the collaborator's side:** a **Shared with you** section lists each organization they help
    with, and opens it clearly marked "**Frank's organization — you are an Approver**", so it is
    never mixed up with their own. Nothing from it is saved on their PC except their own record of
    what they did.
12. **New desktop commands are the main window's alone**, on both sides, with IPC tests that refuse a
    second window, the sign, and a web page: invite, accept, change a role or part, the approvals only
    you can answer, pause, remove, leave, and opening a shared organization.

## Consequences

- A business owner can let a partner or an assistant approve work, from their own PC, without
  sharing a password, a key, or the PC.
- A Viewer reads task conversations, which may hold business details. Choosing the part to share,
  and the role, is how you limit that.
- The Ledger now knows two kinds of people. Every check that looks for the owner gets a test that a
  collaborator is refused.

## Alternatives considered

- **Use the phone's `source = owner`.** Every owner-only check would let a collaborator through.
- **Collaborators through the account service, not live.** Slower answers, and the account service
  would carry approvals, which today never leave your PC and phone.
- **No approvals kept for you to begin with** (as for the phone). The phone is you; a collaborator is
  not, so the safer start fits.
- **A Manager may also make roles, departments, and permissions.** That is "widening what workers may
  do", which the plan keeps with the owner.
