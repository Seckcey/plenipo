# ADR-039: The owner's notes — eight new phases, watching code being written, and the order of work

- **Status:** Accepted (by the owner, 2026-09-28)
- **Date:** 2026-09-28
- **Phase:** plan change, after Phase 13 (v1.9.0, pull request #70)
- **Number:** ADR-037 and ADR-038 are taken by Phase 13's pull request, so this is ADR-039.

> **On screen** (ADR-010, plain words and rank names): nothing changes yet. Each new phase keeps
> the word list (`docs/design/vocabulary.md`). On screen, plugins are **Connections**, a phone
> or web view of Plenipo is the **remote**, the online account is the **8 West account**, and
> watching a worker write code is **Watch**.

## Context

On 2026-09-28 the owner gave a list of notes — one bug, two features, and fifteen improvements —
and left it to this plan where each goes. The owner answered five questions, then added three
answers the same day:

1. **Trash can:** archive, with the ability to delete for good from the archive.
2. **Dragging an agent to another department:** both — move it for good, or lend it for a job.
3. **Project files:** open and edit them in Plenipo, not only look.
4. **Selling Pro:** as soon as possible **after the app is finished** — no hurry now.
5. **The order of work** (the table below): approved.
6. **Watching code being written:** when an agent works in a project file, the owner wants to
   watch the code being written as it happens — "a huge game changer".
7. **Payments:** Stripe. The owner will create a new Stripe account for Plenipo. Pricing and
   selling follow standard practice for a subscription desktop app.
8. **Where the license signing key lives** (Phase 22's open question): left to this plan.

What the code had on 2026-09-28 (read at `b0fba6e`; v1.8.0 on `main`, v1.9.0 in pull request
#70):

- **Canvas** (ADR-009, the organization engine and canvas): a tile can already be dropped on
  another to change who it reports to, but lines cannot be dragged, the layout is automatic,
  the legend has three items, the only canvas filter is search, and there is no trash can.
  Positions are archived, never deleted (ADR-009 §1), and there is no way to bring one back.
- **Properties panel:** one fixed 360-pixel column with thirteen sections. No effort, no
  permissions.
- **Model and effort:** a position can fix its AI tool and model, but not its effort. A role can
  set models and effort. Departments and the organization can set neither.
- **Learning** (ADR-024, workers learn from their work): one switch for everything, plus a
  per-role "keep lessons without asking".
- **Jobs:** twelve built-in roles and no specialties; "Front-end Developer" is only a title.
- **Prompts:** Plenipo's own text is about 3–5 KB per turn (up to 10–12 KB), resent with every
  objective to full-time members (ADR-008 §1), and never measured or scaled to the job.
- **Writing files:** every file a worker writes already passes through Plenipo — its own
  `write_file` and `edit_file` tools (ADR-013) for Claude Code, Codex, and Grok, and ACP's
  `fs/write_text_file` for Kimi (ADR-027). Claude Code also streams each tool call while the
  model is still writing it; Plenipo reads only the text of those streams today.
- **AI tools page:** Re-check only. Sign-in is typed instructions. Token counts are saved per
  turn but never added up. Claude Code's and Grok's self-updaters are off on purpose (ADR-007 §5).
- **Connections:** Plenipo's own tool server speaks MCP (ADR-013), GitHub is the only
  connection, and "add-on tools (MCP servers) you set up" is registered for "a later phase" with
  no phase.
- **Panels and windows:** only the terminal resizes; nothing pops out; one window; no file view.
  The terminal panel already has read-only **watch tabs** for workers on servers (ADR-031).
- **Organizations:** one per Ledger, with no organization column in any table.
- **Licensing:** Phase 11A is planned and not built. Its key is Ed25519, signed with a key
  "that stays offline", and its license service is "separate infrastructure, not built in this
  phase".
- **Platforms:** Windows only, NSIS installer, about 110 platform checks in the code.
- **The bug:** after an update changed how the Ledger is stored, the owner saw "The ledger schema
  was upgraded from version 7…". It worked as designed, but "schema" and the version number
  broke the plain-words rule. Fixed with this record (see §5).

## Decision

### 1. The order of work

Existing phases keep their numbers, because the plan, ADRs, checklists, and reports point at
them. New phases get new numbers. The plan's rule "work only on the earliest incomplete phase"
(§8.3) now means the earliest in this list:

| Order | Phase    | What                                                                                            |
| ----- | -------- | ----------------------------------------------------------------------------------------------- |
| 1     | 13       | Windows install, updates, and recovery (built; merge pull request #70)                          |
| 2     | 17       | The owner's control over workers                                                                |
| 3     | 18       | The organization canvas, and watching workers write code                                        |
| 4     | 19       | The AI tools page: sign-in, usage, and updates                                                  |
| 5     | 16       | Every AI model worth having (ADR-036)                                                           |
| 6     | 20       | Connections: Microsoft 365, Slack, Google, and more                                             |
| 7     | 21       | Workspace: panels, windows, files, and more than one organization                               |
| 8     | 11A + 22 | Free and Pro, the license key, and the 8 West account service (users, billing, email, licenses) |
| 9     | 14       | The remote: CrewOS and a phone app                                                              |
| 10    | 15       | More departments, Windows servers, Milepost                                                     |
| 11    | 9        | Sales on HubSpot                                                                                |
| 12    | 23       | Mac and Linux                                                                                   |
| 13    | 24       | Community                                                                                       |

Everyday work on the owner's own PC comes first. Selling comes once that is done, and before
anything that needs the online service, since the remote and community use the same accounts.
Community is last: it is the largest and the riskiest.

### 2. Decisions that change earlier records

1. **Archive, then delete for good** (amends ADR-009 §1, "archived, never deleted"). Dropping an
   agent on the trash can archives it, with Undo. Archived agents, departments, and projects are
   listed in an Archived drawer and can be brought back. From the drawer, the owner can delete
   one for good, after confirming. The Ledger keeps a short record in its place — name, role,
   and dates — so older activity still reads correctly. Nothing with unfinished work can be
   archived or deleted, as today.
2. **Move or lend.** Dropping an agent on another team moves it for good, as today. A menu
   choice lends it instead: it takes one objective, or works until returned, for the other team
   and then goes home. While lent, it works under the borrowing project's permission limit,
   never its home project's.
3. **The owner opens and edits project files** (widens ADR-009 §7, "the local working directory
   is recorded, never opened"). This is the owner's own action, recorded in the Activity trail.
   ADR-016's "one writer per working copy" stays: a working copy that a worker is writing opens
   read-only until that worker is done, or until the owner stops it. Workers' file access still
   goes through Guard, unchanged.
4. **Agents write short, but in plain words.** Prompts shrink to fit the job. Agents do not
   invent a private language: the owner must be able to read everything workers tell each other
   (plan principle 7, all meaningful work is auditable). Hidden shorthand would also hide
   mistakes and planted instructions, and AI models make more mistakes in made-up shorthand.
   The saving comes from a compact, labeled format and from pointing at saved records instead of
   pasting them again.
5. **Connections live in Plenipo, not in each AI tool.** Every AI tool reaches them through
   Plenipo's own tool server, so every call goes through Guard and every AI tool — including
   Kimi, Grok, and Ollama — can use them. Sign-ins to these services are kept in the Vault.
   Content from email, chat, and files is untrusted input. Sending, deleting, or paying asks
   the owner by default. Connections are compiled into Plenipo or run as separate supervised
   programs; nothing loads code into Plenipo while it runs (ADR-014's rule stays).
6. **Signing in to an AI tool from Plenipo.** A Sign in button opens a terminal tab that runs
   that AI tool's own login command. The owner completes the login, so Plenipo never sees a
   password. ADR-014 §7's rule against Plenipo driving an AI tool's interactive screen
   stays: the owner drives it, not Plenipo.
7. **Plenipo updates the AI tools,** only between tasks, then re-checks each one and asks it for
   its models. The self-updaters stay off, so no tool replaces itself mid-task (ADR-007 §5).
   Models an AI tool reports that Plenipo has not checked yet show as "new — not checked yet"
   and can be used.
8. **Usage only from official sources.** Totals come from Plenipo's own records. A "plan left"
   meter appears only where the AI tool reports it through an official command. Plenipo never
   reads an AI tool's saved sign-in or calls its unpublished web addresses (ADR-007 §4,
   ADR-014 §7).
9. **A phone is a remote.** A phone cannot run the AI tools, so the iPhone and Android app is a
   remote for the owner's PC and joins Phase 14.
10. **One Ledger per organization.** Each organization is its own Ledger file, with its own
    backups, and can open in its own window.
11. **The 8 West account service is its own product, in its own repository.** It holds
    accounts, licenses, billing, and email. Plenipo keeps working without it, as Phase 11A
    requires, and sends it only what Phase 11A allows.
12. **Watch a worker write code, live.** Because every file change a worker makes already
    passes through Plenipo, Plenipo can show it as it happens:
    - **Every AI tool that edits files:** each change appears the moment it is saved, with the
      new and changed lines highlighted, and a list of every file the worker has touched.
    - **AI tools that stream their changes while writing them** (Claude Code does; each other
      tool is checked on its real program): the code appears as the model writes it, marked
      "being written — not saved yet". It turns into "saved" when it lands, or "refused" if
      Guard refuses it.
    - **Where:** a **Watch** tab in the bottom panel, like the server watch tabs (ADR-031), and a
      Watch button on any working agent on the canvas (Phase 18). From Phase 21, the same view
      runs inside the file view and editor: open a file a worker is writing and watch it change.
    - **Read-only:** watching never lets the owner type into a worker's working copy (ADR-016's
      one writer). The tab has Stop, which stops the worker, as elsewhere.
    - **What is kept:** the Ledger records each saved change, as it records tool calls today.
      The letter-by-letter preview is shown, not stored.
13. **Selling with Stripe, the standard way** (for Phase 22).
    - **Stripe Checkout** for buying, **Stripe Billing** for the monthly ($9) and yearly ($99)
      subscriptions (`docs/editions.md`), and **Stripe's customer portal** for changing the card,
      cancelling, and invoices.
    - **Stripe Tax** works out and collects sales tax. Registering in each state where 8 West
      must collect, and filing, stay 8 West's job; Stripe tracks where the thresholds are
      reached.
    - **Stripe's notices to the service** (webhooks), checked for Stripe's signature, drive
      each license's state: paid, renewed, payment failed, cancelled at the end of the paid
      period.
    - **Failed payments:** Stripe retries automatically and emails the customer. Pro stays on
      while Stripe retries, and ends only when Stripe gives up.
    - **Card numbers never touch 8 West.** A separate Stripe account for Plenipo keeps its books
      apart from 8 West's other businesses.
    - No separate Pro trial: the Free edition is the trial.
14. **The license signing key lives in a cloud key vault** (for Phase 22; answers Phase 11A's
    "stays offline"). A service that issues keys when a payment clears must sign on its own, so
    the key cannot stay on a disconnected machine. It lives in a cloud key vault: a service that
    signs on request but never lets the key itself out, not even to 8 West. Only the account
    service may ask it to sign, and every signature is logged.
    - Plenipo carries the current public key and one spare, so the signing key can be replaced
      with an ordinary Plenipo update if it is ever at risk.
    - Phase 11A's key format follows what the chosen vault can sign. It stays Ed25519 if the
      vault supports it, and uses P-256 (another standard signature) if not. This is decided
      before Phase 11A is built, so nothing already released changes.

### 3. What goes where

- **Phase 17:** effort per agent; model and effort rules for the organization and each
  department; learning on and off for the organization, each role, and each agent; specialties
  under each role; archive, bring back, and delete for good; the properties panel rebuilt;
  prompts sized to the job.
- **Phase 18:** the canvas — arranging, dragging lines, move or lend, trash can, toolbar,
  filters, legend, a live view of work, data, and compute, and the owner's own tile (avatar,
  status light, mood, short message) — and **watching workers write code** (§2.12).
- **Phase 19:** the AI tools page — sign in, reconnect, sign out, usage, the payment method,
  updates, and new models.
- **Phase 16:** moves up, after Phase 19. Its paid keys fill the payment-method switch that
  Phase 19 puts on each AI tool.
- **Phase 20:** Connections, with Microsoft 365 first.
- **Phase 21:** panels that resize, dock, and pop out; a file view with an editor, where
  watching a worker write happens right in the file; more than one organization.
- **Phase 11A and the new Phase 22:** selling Pro once the app is finished — the license key in
  the app, and the service that sells it with Stripe and checks it (§2.13, §2.14).
- **Phase 14:** gains the phone app.
- **Phase 9:** after Phase 20, so HubSpot is a Connection first.
- **Phase 23:** Mac and Linux.
- **Phase 24:** community — linked organizations, messages between people, and collaborators.

### 4. Each phase still writes its own records

This record sets the order and the answers above. Each phase writes its own ADR for its design
choices when it starts, as every phase has.

### 5. The bug, fixed now

The notice now reads: "Plenipo updated the Ledger, where it keeps your activity history, for this
version. Nothing was lost: a copy of the old Ledger was saved first, in …". The version number
stays in the backup's file name.

## Consequences

- **The next six phases are local.** They improve what the owner uses every day, with no new
  online service and no new permissions for workers.
- **Selling starts once the everyday app is done,** with the account service built alongside
  Phase 11A. The remote and community come after, on the same accounts.
- **Watching code needs no new permission.** It shows what Plenipo already sees. The
  letter-by-letter view depends on each AI tool streaming its changes; where one does not, the
  owner still sees every change the moment it is saved.
- **Deleting for good is new** and can remove records the owner might want later, so the drawer
  asks first, and the Ledger keeps a short record in the deleted item's place.
- **Owner file editing and working copies can collide.** The one-writer rule settles it, and the
  owner can see which worker holds a working copy.
- **A phone app, more windows, and Connections each widen what can reach Plenipo.** Each window
  gets its own permission file (ADR-033's rule), each Connection goes through Guard, and the
  remote keeps Phase 14's local policy.
- **The signing key is online, in a vault.** It is safer than a key on a laptop, but it means the
  account service's own sign-in to the vault must be guarded like the key itself.
- **Community brings other people's words to the owner's workers.** It waits until last and
  gets its own safety rules.
- **Phase numbers no longer run in order.** The order-of-work list at the top of the plan is the
  guide.

## Alternatives considered

- **Renumber the phases to match the order.** Rejected: dozens of documents point at the current
  numbers.
- **Sell first, right after Phase 13** (this record's first draft). Rejected by the owner:
  selling waits until the app is finished.
- **Keep the old order: remote next, the owner's notes later.** Rejected: the owner's everyday
  control comes first.
- **Let agents invent a compact language.** Rejected: the owner could not read it, and it would
  hide mistakes and planted instructions.
- **Watch by reading the worker's screen or the AI tool's own display.** Rejected: Plenipo
  already has every change in hand, in order, with its file name.
- **Use each AI tool's own connectors** (Claude's or Codex's). Rejected: only that tool could
  use them, Guard would not see the calls, and Plenipo already runs Claude Code with its own
  tools only.
- **A full Plenipo on phones.** Not possible: a phone cannot start the AI tools' programs.
- **Delete on the trash can, with no archive.** Rejected by the owner, who chose archive first.
- **Let the AI tools update themselves.** Rejected: a tool could replace itself mid-task, and
  its output could change under a running parser.
- **A merchant of record instead of Stripe** (a company that sells on 8 West's behalf and files
  sales tax, which ADR-022 weighed). Not chosen: the owner uses Stripe, and Stripe Tax covers
  working out and collecting the tax.
- **Keep the signing key offline and sign each key by hand.** Rejected: customers would wait for
  a person to issue their key.
