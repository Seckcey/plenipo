# ADR-021: On/off switches in Settings

- **Status:** Accepted (by the owner, 2026-09-27)
- **Date:** 2026-09-27
- **Phase:** 10 (follow-up, v1.4.0)
- **Amends:** ADR-020 (Plenipo's browser and computer use), sections 4, 5, and 6

## Context

When the owner accepted ADR-020 (Plenipo's browser and computer use, through Guard), they asked
for on/off switches in Settings for its features, and then for switches for sending, buying,
signing in, CAPTCHA attempts, and screenshots. Their rule for the website switches: **"if it's
toggled on, agents don't need to ask. If it's toggled off they have to ask."**

This changes ADR-020 section 4, where submitting, buying, signing in, and sending **always**
wait for the owner, and the owner's earlier Phase 10 rule saying the same. The owner is changing
their own rule, so the switches are theirs to set, but three risks need safeguards:

- A website can put words on a page that trick a worker (prompt injection). A worker that no
  longer asks before sending or buying could be led to send or buy on a site's say-so.
- "Signing in without asking" must not mean a worker types passwords; ADR-020 section 5 keeps
  passwords and secrets out of workers' hands.
- "CAPTCHA attempts": a worker trying to solve or retry a CAPTCHA gets past a website's check
  that a person is present. That is bypassing site security. It breaks most websites' terms,
  can break computer-misuse laws, and breaks the owner's own Phase 10 rule ("never bypass
  CAPTCHAs or site security") and the rollout plan's out-of-scope list. It is **not built**.
  What the owner wants from it, that work doesn't stop dead at a CAPTCHA, is met by handing the
  check to the owner instead.

## Decision

A **Switches** section in Settings (after Personalization), stored in the Guard settings
(`switches`, Ledger setting `guard`, event `guard.switches_changed`):

| Switch (on screen)                              | Starts | Effect                                                                                          |
| ----------------------------------------------- | ------ | ----------------------------------------------------------------------------------------------- |
| Plenipo's browser                               | On     | Off: no worker gets `browser.navigate` or `browser.automate`, whatever its permissions.         |
| Screen, mouse, and keyboard                     | Off    | Off: no worker gets `computer.observe` or `computer.control`. The last resort, so off to start. |
| Sending forms and messages (without asking)     | Off    | On: sending goes ahead without an approval, on allowed websites only.                           |
| Buying and paying (without asking)              | Off    | On: buying goes ahead without an approval, on allowed websites only.                            |
| Signing in (without asking)                     | Off    | On: pressing Sign in goes ahead without an approval, on allowed websites only.                  |
| Hand me checks that a person is using a website | On     | On: a worker can hand a CAPTCHA to the owner (below). Off: the worker stops and says so.        |
| Screenshots in the Activity trail               | On     | Off: steps keep no picture. Approval cards always keep theirs.                                  |
| Worker learning                                 | On     | ADR-022 (workers learn from their work).                                                        |

### 1. A feature switched off

- Guard's level for that capability is **Blocked** (layer: rule), whatever a role's
  permissions, with the reason "Plenipo's browser is switched off (Settings → Switches)" or
  "the screen, mouse, and keyboard are switched off (Settings → Switches)".
- Switching it off **stops** every worker using it now (`control.switched_off`; their tabs are
  released and what they were waiting for is refused). Unlike the emergency Stop, it does not
  keep other features from starting.
- A worker given no tools because of a switch is told why (`guard.grant_skipped`).

### 2. Without asking, and its safeguards

A sensitive browser action (sending, buying, signing in) goes ahead without an approval only
when **all** of these hold:

- its switch is on;
- the capability is `browser.automate` (never the desktop's mouse and keyboard);
- the page's website is on the owner's **Allowed** list (not just "not blocked");
- the owner's rule for that sensitive kind is **Ask** (a rule set to **Blocked** stays blocked);
- the role's own level is not "ask me" (a role set to ask still asks).

Data the page sends after such an action (ADR-020 section 4's network gate) is released the same
way when every website involved is allowed. Each step is still recorded in the Activity trail
with its screenshot (if screenshots are on), and the approval note says the owner let workers do
this without asking.

Fixed on the way: with sending set to **Blocked**, data a page tried to send asked the owner
instead of being refused.

### 3. Signing in

"Signing in without asking" covers pressing **Sign in** or **Log in** only. Workers still never
type into password, one-time-code, or card fields, and never type a secret: the owner signs in
in Plenipo's browser, whose own profile then keeps the site's sign-in.

### 4. Checks that a person is using a website (CAPTCHAs)

- New tool `browser_person_check` (capability `browser.navigate`), used when the page shows a
  CAPTCHA. The worker gives its reason; Plenipo asks the owner, with a screenshot, to **solve
  the check themselves**.
- The page comes to the front with a **purple** sign: "Please solve this check yourself, then
  press Approve in Plenipo. {worker} waits." The owner's clicks on that page do not count as
  taking over (mode `handed`).
- When the owner presses **Approve**, the worker gets the page back and reads it again. If they
  press **Deny**, the worker is told the owner did not solve it and stops.
- The worker **never** clicks, types, or presses keys in a CAPTCHA, never retries one, and never
  looks for a way around it. The tool refuses when the page shows no CAPTCHA.
- With the switch off, the tool refuses and the worker stops and says so (ADR-020's behavior).

### 5. What has no switch

Always on: workers never type passwords or secrets and never try to get past a CAPTCHA; the
sign shows whenever a worker uses the browser or the owner's mouse and keyboard; **Take over**
and the emergency **Stop** work at once. Taking control of the mouse and keyboard, and pressing
Enter while holding them, always ask.

## Consequences

- The owner decides how much the browser asks them, per kind of action, and can switch whole
  features off with one click.
- With a "without asking" switch on, **a website the owner allowed could trick a worker into
  sending, buying, or pressing Sign in**. Settings says so in a notice above those switches. The
  allowed list, the Blocked rules, and the role's "ask me" level are the remaining guards.
- Screenshots off means the Activity trail has no pictures of steps. Approvals still show one.
- The screen, mouse, and keyboard start **off**. An owner who gave a role the "Screen, mouse,
  and keyboard" permission set in v1.3 turns this switch on after updating (no built-in role
  has it).
- CAPTCHAs no longer end the work: the owner solves them when asked. The worker waits up to
  the approval's time limit.

## Alternatives considered

- **Let workers attempt CAPTCHAs (up to three tries).** Rejected: it is bypassing a website's
  security. It breaks sites' terms, risks the owner's accounts being banned, can break
  computer-misuse laws, and breaks the owner's own rule. Solving services are rejected for the
  same reasons.
- **"Without asking" on any website that is not blocked.** Rejected: unknown websites are the
  likeliest to trick a worker. Only the Allowed list, which the owner curates, qualifies.
- **A switch for the sign, Take over, or Stop.** Rejected: they are how the owner sees and stops
  a worker, and cost nothing when unused.
- **Per-website switches.** Deferred: the Allowed list already scopes them. Per-site rules can
  come later if the owner needs them.
