# ADR-085: Paid AI keys with spending caps — the switch, caps for the business, a department, and a position, and a record of every paid task

- **Status:** Proposed (2026-09-30), built at the owner's direction with the owner's choices (the
  [Wave 3 checklist](../phases/phase-16-wave-3-checklist.md#the-owners-choices-2026-09-30)):
  "All are approved except the following. 8. No limits 13. Yes you can create and merge and run
  any actions needed. 14. I have keys for anthropic, openai, grok and kimi."
- **Date:** 2026-09-30
- **Phase:** 16, Wave 3 ("spending caps first, then paid routes")
- **Follows:** [ADR-036 (every AI model worth having)](ADR-036-every-ai-model.md) §2 (keys and
  caps) and §4 (more than one route to a model).
- **Amends:** [ADR-003 (roles never tied to one AI company)](ADR-003-provider-independent-roles.md),
  [ADR-007 (how Plenipo runs AI tools)](ADR-007-runtime-adapters.md) §4 (sign-ins and billing),
  [ADR-011 (how Plenipo picks each worker's model)](ADR-011-model-policy-routing.md), and
  [ADR-014 (adding AI tools)](ADR-014-adding-ai-tools.md) bar item 3 (subscription sign-in only).
  It amends them; it does not rewrite them (§8).
- **With:** [ADR-086 (OpenRouter through a Plenipo helper)](ADR-086-openrouter-through-a-plenipo-helper.md)
  and [ADR-087 (direct keys for every AI company whose models take one)](ADR-087-direct-keys-for-every-ai-company.md).

> **On screen** (ADR-010, plain words and rank names): **Spending caps**, **a monthly cap**, **the
> business's cap**, **Let workers use paid AI keys**, **set aside**, **not priced yet**, **80% of
> a cap is used**, and **Paid AI work stopped**. Never "budget", "quota", "BYOK", "metered",
> "reservation", or "hard limit".

## In short

Plenipo can now spend the owner's money, but only inside fences the owner sets first:

1. A switch, **Let workers use paid AI keys**, off by default. Off, Plenipo behaves exactly as it
   did before Wave 3.
2. **Spending caps**: a monthly amount for the whole business, a department, or one position. No
   paid key works until the business's cap exists.
3. Before a paid task starts, Plenipo **sets aside** the most it could cost, and never starts a
   task that could pass a cap. So the hard stop never goes over; work can stop a little before
   100% instead.
4. Every paid task is **recorded** in the Ledger: what it cost, against which caps, and which key
   by its name. A task whose bill cannot be read counts at the most it could have cost ("not
   priced yet"), never as zero.
5. At 80% of a cap the owner is warned, and when a cap stops paid work the owner is told: each
   once a month, as a Windows notice and a banner on every page.

## Context

ADR-036 allowed paid AI keys, off by default and never without a spending cap, and asked that the
caps be built before the first key works. It left four older records to be amended "by its wave's
own record": ADR-003 (no silent paid fallback), ADR-007 §4 (sign-ins and billing), ADR-011 (paid
use fixed off), and ADR-014 (subscription sign-in only). This is that record.

The owner's choices for Wave 3 (2026-09-30), from the checklist:

1. **The business cap first.** No key can be saved until the whole-business cap exists.
   Department and position caps are extras; every paid task counts against its position, its
   department, and the business, and the smallest amount left decides.
2. **The month** starts on the 1st at midnight, Pacific time.
3. **The hard stop never goes over.**
4. **No known price, no paid key.**
5. **Direct keys run through Plenipo's own helper**, not through the companies' own programs.
6. **Paid workers answer in text only** in this wave.
7. OpenRouter: a short checked list, plus any model the owner names.
8. **Privacy on OpenRouter: no limits.**
9. to 14.: how the work is done, and direct keys for every AI company whose models take one.

## Decision

### 1. The switch

`Switches.paidAiKeys` (Settings → Switches, ADR-023): **Let workers use paid AI keys**, off by
default. A setting saved before Wave 3 reads it as off. While it is off, no key can be saved, no
paid route is offered, and no paid task runs; every test that forbids keys passes unchanged.

### 2. Spending caps

1. **What a cap covers:** the business (every paid task), a department (its positions' paid tasks,
   worked out from who each reports to, or the team an agent is lent to), or one position. One cap
   each; a department or position must be in the organization.
2. **How much:** a monthly amount from $0.01 to $1,000,000. The warning at 80% and the hard stop at
   100% are fixed, not settings.
3. **The month** is the calendar month in Pacific time: it starts over at midnight on the 1st,
   daylight saving time included (United States rules since 2007, in one small function).
4. **The business cap first:** without it, no paid task starts (and, from part 2, no key can be
   saved). The business cap cannot be removed while a paid key is saved.
5. **The gate.** Before a paid task's request is sent, Plenipo adds up, for the task's position,
   its department, and the business: what this month has spent, what is set aside for running
   tasks, and the most this task could cost. If that does not fit under every cap, the task does
   not start, and the reason names the cap: "Not started: this task could cost up to $3.00, and
   $1.20 is left this month under the business's spending cap ($50.00 a month)." Otherwise the
   amount is set aside. Checking and setting aside happen in one Ledger transaction, so two tasks
   can never both fit into the same last dollars.
6. **After the task,** its record says what it really cost, and the rest is freed. A bill larger
   than what was set aside (a price that changed) is recorded as it was, and only then are the
   caps it passed named, for the caller to stop that task's work at once. A bill beyond any cap is
   a mistake somewhere: it is recorded at the most any cap could be (which stops paid work), with
   a note, never refused, so a known bill is never counted as less.
7. **Telling the owner:** the first time in a month that a cap reaches 80%, and the first time a
   cap stops paid work (used up, or a task that did not fit), an event becomes a Windows notice
   (Settings → Notifications → **Paid AI spending**) and a banner on every page, with
   **Spending caps** to open the page. The banner stays until the cap is raised or the month
   starts over (it looks again then by itself). Raising a cap that stopped work lets work go on at
   once. A late bill from an earlier month counts in its own month and changes nothing about this
   month's warnings.
8. **In refusals,** what a task could cost is rounded up to the cent and what is left rounded
   down, so a refusal never reads as though the task would have fit.

### 3. Money and prices

1. **Money is whole millionths of a dollar** ("micros", `u64`), never a floating-point number.
2. **A price** is per million tokens, in micros, for input, cached input, and output
   (`plenipo_runtime::pricing::Price`). The most a request could cost is its input tokens plus its
   longest allowed answer; a finished request is priced from its token counts. Every part is
   rounded **up**.
3. **A service's own bill** (OpenRouter's `usage.cost`) is read exactly from its decimal text, and
   rounded up past the sixth decimal. OpenRouter's per-token prices ("0.000003") become prices per
   million tokens the same way.
4. **No known price, no paid key** (choice 4): the Router skips such a route and says "not priced
   yet". A finished task whose bill could not be read is recorded as **not priced yet** and counts
   at the most it could have cost (what was set aside), never as zero (ADR-036 §2.5).
5. **Cached input** is never priced above other input: a price list that says otherwise is
   refused, because the most a request could cost counts every input token at the input price. A
   report with more cached tokens than input tokens counted them apart, so all of its input is
   priced as fresh and its cached tokens on top. Plenipo's helpers never ask a service to store a
   conversation for reuse (a "cache write"), which some services price above ordinary input.

### 4. The record of every paid task

Ledger migration 0012 adds `spending`: one row per paid task, with its task and execution, the
position and department (their IDs and their names as they were, since both can be removed from
the organization later), the AI tool and model, the key by its reference ID and name (never the
key), the Pacific month, the amount set aside, what it cost or that it was not priced, how it was
priced (the service's bill or the price list), and when. A row is never deleted, and a settled row
never changes (triggers refuse both). Every step is an event in the task's own trail
(`spending.set_aside`, `spending.recorded`), and caps' changes are events too
(`spending.cap_set`, `spending.cap_removed`).

**After a restart,** money still set aside from before this run began belongs to tasks that
stopped with Plenipo, whose bills were never read. Each is recorded as not priced yet and counts at
the most it could have cost. It is never freed, so a cap is never passed unseen. This runs once the
notices have started (so a cap it reaches is told), and a task this run started is left alone.

**After a restore** of the Ledger from a backup, the Ledger as it was is kept as a backup, and its
spending records are carried into the restored Ledger: money spent since the backup was made still
counts, and restoring never opens room under a cap.

**If the caps cannot be read** (a damaged setting, or an older Plenipo reading a newer one's), no
paid task starts, a bill is still recorded, and Plenipo says so with two ways out, like its other
settings: restore a backup of the Ledger, or reset spending caps to none (after a backup) and set
them again. What was spent is kept either way. The Ledger's export includes every spending
record.

### 5. Paid keys (part 2)

Keys are kept in the Vault (Windows Credential Manager), as server sign-ins are, under their own
names (`paid-key-…`). Settings keep a name, the AI tool it is for, and when it was saved: never the
key. A key is typed only into Plenipo's own screen, checked with one cheap read call, never shown
again, and added to the secret filter, so it cannot reach a log, the diagnostics file, the Ledger,
or a task's activity. It reaches only its own helper, **on the helper's standard input**, only for
that AI tool, only while the switch is on: never in an environment variable or on a command line.
So the contract suite's "no key variables" check stays unchanged for every AI tool, subscription or
paid (stricter than ADR-036 §2.4's "declares the variables it needs").

### 6. Routes (part 2)

A route is one model, the AI tool that runs it, and how it is paid for (a subscription or a named
paid key). The same model on different AI tools is shown as one model with more than one way to
reach it (each AI tool's list names it by one short name, such as `kimi-k3`, and its card says
where else it runs). A position's list is in the owner's order. **A paid route is used only where
the owner listed it** (a position's, role's, department's, or the business's list, or a position
set to always use it), never picked from the whole list of models, so with no order set,
subscriptions come first (ADR-036 §2.6) and no paid route runs at all. The Router moves to the
next route when one is usage-limited, signed out, over its cap, not priced yet, or has no key, and
its reason names the route it chose, says whether it costs money ("It costs money: it is paid per
use with your key, within your spending caps."), and says "A worker on it answers in text only."
for a route without tools. A paid AI tool is ready only with its key: a check that claims a
subscription for it is not believed.

### 7. Paid helpers (parts 2 and 3)

Paid routes run through a Plenipo helper per task, like the Ollama helper (ADR-017): OpenRouter
(ADR-086) and each AI company's own service (ADR-087). Each reaches only fixed addresses, which
Guard checks before the helper starts and the helper checks again; redirects are not followed.
Workers on a paid route answer in text only in this wave (choice 6).

### 8. What this amends

- **ADR-003 (roles never tied to one AI company).** "Paid API fallback is disabled unless the
  owner enables it" becomes: paid use happens only while the switch is on, only on routes the
  owner put on a position, only within the caps, and the Router's reason says when a route costs
  money. Nothing turns subscription use into paid use silently.
- **ADR-007 §4 (sign-ins and billing).** Unchanged for every AI tool that signs in with a
  subscription: key variables are never passed and an API-key sign-in is still refused as
  `billingNotAllowed`. New: a paid helper receives the owner's saved key from the Vault on its
  standard input, for its own task only.
- **ADR-011 (how Plenipo picks each worker's model).** "API use allowed/disabled is global and
  fixed off" becomes the owner's switch; the Router skips a paid route that is over a cap, not
  priced yet, or has no key, and says why. The Router still skips a _subscription_ AI tool signed
  in with an API key: a paid key is only ever used through a paid route.
- **ADR-014 (adding AI tools), bar item 3.** For an AI tool that signs in with a subscription,
  unchanged. A paid AI tool is allowed through a Plenipo helper when its key is typed into
  Plenipo's own screen and kept in the Vault, its requests are priced (or counted at the most they
  could cost), and a spending cap covers every task.

## Consequences

- **Plenipo can spend the owner's money,** and every dollar is fenced: no key without the
  business cap, no task that could pass a cap, and a record of every paid task.
- **Work can stop a little before a cap is used up**, because the most a task could cost is set
  aside first. That is the price of never going over.
- **A crash can make spending look higher than it was** (a task counted at the most it could have
  cost), never lower.
- **The owner is told once a month per cap,** not per task; the banner stays until the owner acts.
- **Prices can go stale.** Built-in prices carry the date they were checked; OpenRouter's come from
  its public list before each task.
- **To watch** (ADR-036): where prompts are processed (OpenRouter with no limits, choice 8), and 8
  West IT client data, which reaches a paid route only through a position the owner put it on.

## Alternatives considered

- **Warn at 100% and stop the next task** (let the running request finish). Rejected by the owner
  (choice 3): it can go over.
- **Free what was set aside after a restart.** Rejected: the service may have billed the request;
  counting at the most it could have cost keeps the cap honest.
- **Floating-point dollars.** Rejected: sums drift, and a cap check must be exact.
- **Keys in environment variables** for paid tools, as ADR-036 §2.4 allowed. Rejected for
  something stricter: standard input reaches only the helper, and the contract suite needs no
  exception.
- **A cap per key** instead of per business, department, and position. Not asked for; the three
  levels ADR-036 names cover who spends.

## As built

**Part 1 (2026-09-30), spending caps, pricing, records, and the switch:**

- `crates/ledger/src/spending.rs`: caps (setting `spending`), the gate
  (`set_aside_spending`), the bill (`settle_spending`), restart handling (`recover_spending`), the
  Spending caps page, Pacific months, and `dollars()`; migration `0012_spending`.
- `crates/runtime/src/pricing.rs`: `Price`, the most a request could cost, a bill from token
  counts, and exact decimal dollars.
- `crates/guard/src/dto.rs`: `Switches.paid_ai_keys`, off by default.
- `crates/ledger/src/notices.rs`: the notice kind **Paid AI spending** (on by default).
- Desktop: `get_spending`, `set_spending_cap`, `remove_spending_cap` (main window only);
  Settings → **Spending caps**; the switch in Settings → Switches; the banner on every page; the
  notice choice in Settings → Notifications. On start, money left set aside is counted (§4).
- The AI tools page's paid-key switch stays locked ("Comes in a later update, within your spending
  caps") until part 2. (Part 2 replaced it: a subscription AI tool's card says it always uses the
  subscription, and a paid AI tool's card has the key form.)
- Tests: the gate and the hard stop, caps for the business, a department, and a position, 80% and
  100% once a month, a bill over what was set aside, raising a cap, not priced yet, restart
  handling, settled rows never change, Pacific months and daylight saving time, exact prices;
  IPC tests that refuse the sign window, other windows, and web pages; the screens.
- **The review** (four reviewers — money and caps, security and desktop commands, the Ledger's
  data, and the screens — with each finding checked by a second reviewer; details in the
  [checklist](../phases/phase-16-wave-3-checklist.md#review-of-part-1)) led to: marks for the
  current month only (§2.7); "passed" only for a bill over its set-aside, and a bill beyond any
  cap recorded, never refused (§2.6); refusal money rounded the safe way (§2.8); cached input
  never above input, and no cache writes (§3.5); a bill recorded even when the caps cannot be
  read, with a reset in Settings; spending kept through a restore; recovery after the notices,
  leaving this run's tasks alone (§4); spending in the export; and on the page, a cap on an
  inactive department shown, positions named with their department, amounts with commas only in
  groups of three, and the page and banner looking again when the month starts over.

**Part 2 (2026-09-30), keys, routes, and OpenRouter** (the helper itself is in
[ADR-086](ADR-086-openrouter-through-a-plenipo-helper.md#as-built)):

- Keys (§5): `crates/capabilities/src/paid/mod.rs` saves a key only while the switch is on and the
  business cap exists: into the Vault first, then OpenRouter checks it, then its reference is kept
  in Guard's settings (name, AI tool, times), and a refused key is erased again. Replacing a key
  erases the old one; removing it erases it from the Vault. Paid keys are in the secret filter
  (`Broker::refresh_redactor`) and in `vault::stored_ids`, so Uninstall's "delete my data"
  removes them. The business cap cannot be removed while a key is saved (IPC test).
- The gate (§2.5): before each step of a paid task, the runtime asks the paid gate
  (`PaidGate`) to set aside the most the step could cost at the model's price, for the task's
  position and department (from the task's own record) and the business; after the step, the
  service's bill (or the price list's) settles it. A step that could not start is not charged.
- Routes (§6): `RouteInput.spending_room` (the smallest amount left under the caps covering the
  work, `Ledger::spending_room`), `RouteChoice.paid`, and the reason's words; a paid route
  skipped when not priced or when nothing is left, and never taken from the whole list.
- Screens: OpenRouter's card on the AI tools page (**Key check**, **Comes with Plenipo**, the key
  form with **Save and check**, **Replace key**, and **Remove key** that asks first, and a notice
  with **Open Switches** and **Open Spending caps** while paid keys cannot be used), prices on
  paid models ("$3.00 a million tokens read, $15.00 a million written"), and "also on …" for the
  same model on other AI tools.
- Tests: the Router (a paid route only where listed; the owner's example, Kimi usage-limited and
  the Ollama plan carrying the work; a paid route next, with "It costs money"; skipped over its
  cap, not priced, or without its key), the contract suite with a paid stand-in (the key only on
  standard input, every step set aside and settled), keys saved, refused, replaced, and removed,
  IPC tests for the key commands, and the screens.
- **The review** (five reviewers — secrets and the Vault, money and caps, Guard and the network,
  desktop commands and routes, and the screens — with each finding checked by a second reviewer;
  details in the [checklist](../phases/phase-16-wave-3-checklist.md#review-of-part-2)) led to:
  each request carrying the price set aside as its most, and models with other fees not priced
  (§3.4); the whole bill when the owner's own company keys are used on OpenRouter (§3.3); a paid
  route skipped when less is left than its shortest task (§6); OpenRouter checked again when paid
  keys or the business cap change, and after the gate exists at startup; keys erased before they
  are forgotten, saves taking turns, and the replaced key kept listed until the new one's check
  passes (§5); and on screen, Windows Credential Manager by name.

**Part 3 (2026-09-30), direct keys** ([ADR-087](ADR-087-direct-keys-for-every-ai-company.md#as-built)):
ten AI companies' own services join OpenRouter as paid AI tools, each with its models and the
prices on its own pages. A price can now carry what a company charges for storing input for reuse
by itself (`Price::cache_write`), counted for every fresh input token (§3).
