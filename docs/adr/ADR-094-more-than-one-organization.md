# ADR-094: More than one organization — each its own Ledger, window, backups, and Vault names

- **Status:** Accepted (2026-09-30). It carries out the owner's answers in
  [ADR-091 (what the check found, and the owner's answers)](ADR-091-phase-21-owners-answers.md);
  the owner reviews it with Phase 21's pull request.
- **Date:** 2026-09-30
- **Phase:** 21
- **Carries out:** [ADR-045 (experience and your Workforce)](ADR-045-experience-and-the-workforce.md)
  §9 and §11 (a whole organization deleted for good, with the same offer; the Workforce is yours,
  not one organization's); [ADR-056 (your tile)](ADR-056-the-owners-tile.md) §5 (your tile is
  yours, not one organization's)
- **Amends:** [ADR-013 (Guard and the Vault)](ADR-013-guard-capability-broker.md) (the Vault's
  names, per organization); [ADR-037 (background work)](ADR-037-background-work.md) (backups and
  restore, per organization; Stop all and Quit, for every organization);
  [ADR-021 (Free and Pro editions)](ADR-021-editions-and-license.md) (one organization in Free)

> **On screen** (ADR-010, plain words and rank names): **organization**, **New organization**,
> **Use a template**, **Copy from one of your organizations**, **Start from scratch**, **Switch
> to**, **Open in a new window**, **Archive organization**, **Bring back**, **Delete for good**.
> Not "tenant", "workspace", "instance", "database", or "profile".

## In short

Plenipo can run more than one organization, for example your own and a client's. Each
organization has **its own Ledger** (its record of everything), its own backups, its own working
copies and screenshots, and its own secrets in the Vault. Each window shows **one** organization;
**Open in a new window** puts a second one beside the first, and both can work at the same time
without ever mixing. The PC's own things are shared: the AI tools and their sign-ins, your
Workforce, your tile, your notice and terminal choices, the screen and mouse, and Plenipo's
updates. **Accepting this record means** building it as written below.

## Context

Phase 21 of `ROLLOUT_PLAN.md`: "create, rename, switch, and open in a new window; each window
belongs to one organization", with "one Ledger file per organization (ADR-039 §2.10), each with its
own backups", "AI tool sign-ins belong to the PC and are shared", and "the Vault keeps each
organization's secrets under that organization's name". ADR-091 lists what the check found (items
8 to 14) and the owner's answers (items 2 to 6).

## Decision

### Where each organization lives

1. **Its own folder.** The first organization stays exactly where it is (Plenipo's data folder:
   `ledger\plenipo.db`, `working-copies`, `screenshots`, and the rest), so nothing moves when
   Plenipo is updated. Every other organization gets a folder of its own,
   `organizations\<its ID>\`, laid out the same way: its Ledger, its backups beside it, its
   working copies, screenshots, attachments, Plenipo's browser profile (its website sign-ins), and
   the tickets its AI tools use to reach Plenipo's tools.
2. **The list of organizations** is `organizations.json` in the data folder: each one's ID, name,
   when it was made, and whether it is archived, plus which one the first window shows. Each
   organization's name is kept in its own Ledger too; the list is a copy, so Plenipo can show the
   names without opening every Ledger.
3. **Every organization that is not archived is open while Plenipo runs,** with its own full set:
   its Ledger, the Supervisor that runs its programs, its AI tools' sessions, Liaison, the Router,
   Guard and its tool server, its notices, and its org chart. Its work keeps going with or without a
   window, as it does in the tray (ADR-037).

### What the PC shares

4. **The AI tools** (ADR-091 §4): each AI tool's sign-in stays with the AI tool itself, and
   Plenipo's own folder for the tools that keep their settings with Plenipo
   (`runtime\ai-tool-homes`) is the same for every organization. The AI tools page, its facts
   (versions, updates, usage, models), and Plenipo's updates are the PC's, kept with the first
   organization. An AI tool's update waits for the first organization's workers (as before), and
   while it runs, every other organization's new work for that tool waits too: Plenipo looks at
   the updates going on each second and holds the tool in the other organizations. It does this
   beside the AI tools page, without changing it (ADR-090: Phase 16's second wave owns that page).
5. **Your Workforce and your tile** are kept in the first organization's Ledger, the PC's shared
   record. Every organization shows the same Workforce and the same tile, and hires from it.
   - Saving an agent to your Workforce from another organization moves it into the shared record,
     with its role's and specialty's names, so any organization with a role of that name can hire it.
   - Hiring moves it out of the shared record into the organization that hired it; it is recorded
     there, as before (ADR-045).
   - An agent whose role the first organization does not have stays in its own organization's
     Workforce, and only that organization shows it (a limit, below).
6. **Your choices for the PC:** notice choices, the terminal's shell, and start and close are kept
   with the first organization and copied to every other one when they change, so each
   organization's notices and terminals follow them.
7. **The screen, mouse, and keyboard** are the PC's (ADR-091 §3): one record of who uses them, for
   every organization, drives the sign, the tray, and **Stop all**.
   - **Stop all** stops every worker using the browser, the screen, or a server, in every
     organization, and each organization's Ledger records its own part. **Allow again** allows it
     in every organization.
   - **Quit** stops every organization's work the normal way, and each Ledger records it.
     Closing the window hides Plenipo to the tray while any organization has work going.

### What each organization keeps

8. **Everything else is the organization's own** (ADR-091 §4): the org chart, departments,
   projects, work, approvals, permissions, switches, AI model choices, servers, connections,
   lessons, the Activity trail, backups, working copies, and screenshots.
9. **The Vault keeps each organization's secrets under its own name.** The first organization
   keeps the name it has (`com.eightwest.plenipo`); another organization's secrets are kept under
   `com.eightwest.plenipo.org-<its ID>`. Two organizations connected to the same service keep two
   sign-ins that never overwrite each other. The uninstaller forgets every organization's secrets
   when asked to (Phase 13).
10. **Backups are the organization's own** (ADR-037): the daily backup, the one before an update,
    and the ones made by hand are kept beside each organization's Ledger. Diagnostics lists and
    restores the backups of the organization its window shows, and a restore never touches another
    organization's Ledger. Restoring still restarts Plenipo (ADR-091, item 13): the other
    organizations' work stops the normal way first and is recorded, and Plenipo asks before it
    restarts.

### Windows

11. **Each window shows one organization, and an organization shows in one window at a time.** The
    first window shows the organization chosen last. **Switch to** shows another organization in
    this window: the page loads again, and this window's terminals and pop-outs close (ADR-091
    §11). **Open in a new window** opens the organization in a window of its own. Asking for an
    organization already open in a window brings that window to the front.
12. **A window's commands act on its own organization only.** Plenipo finds the organization by
    the window that called, never by anything the page sends, so no page can reach another
    organization's work, approvals, or secrets. Live updates go to the organization's own window
    only (ADR-091 §10).
13. **Permission files** (ADR-091 §9): every organization's window has the same panels, so they
    share one permission file (`default.json`, for `main` and `org-*`); pop-outs have their own
    with no commands (ADR-092); the sign has its own, as before.
14. **Closing an organization's own window** does not stop its work, and the organization can be
    opened again from any window. Closing the first window works as before (ADR-037).

### Making, renaming, archiving, and deleting

15. **New organization** asks for a name and how to start (ADR-091 §5):
    - **Use a template:** shown, with "Templates are coming later", until the first template is
      added. Plenipo's code keeps a place for them.
    - **Copy from one of your organizations:** copies its setup: its departments and positions
      (the org chart, with no one hired and no work), roles and specialties, permission sets and
      switches, AI model choices, learning choices, and titles. It never copies work, the Activity
      trail, approvals, lessons, experience, projects, servers, connections, websites' sign-ins, or
      any secret (a permission set that names a secret keeps the name, without the secret).
    - **Start from scratch:** Plenipo's starting settings, as a new installation has.
      The new organization opens in a new window.
16. **Rename:** Settings → Organization renames the organization the window shows, as before; the
    list follows.
17. **Archive organization** (ADR-091 §6) stops its work, closes its window, and hides it from the
    list; **Bring back** opens it again. Nothing with unfinished work can be archived.
18. **Delete for good** (from the archive) asks first and offers to save its experienced workers to
    your Workforce, as ADR-045 §8 does for a department or project. It then removes the
    organization's folder (its Ledger, backups, working copies, screenshots, and browser profile)
    and forgets its secrets in the Vault. Project folders you chose are yours and are never
    deleted. The first organization's Ledger records that it was deleted, and by whom.
19. **The Free edition keeps one organization; Pro has as many as you want** (ADR-091 §2).
    `docs/editions.md` gains the row; nothing is enforced until Phase 11A's editions exist.

## Limits

- **The first organization cannot be archived or deleted.** It holds the PC's shared record (your
  Workforce, your tile, the AI tools page, and your choices for the PC). Moving that record to a
  file of its own is left for later.
- **An agent whose role only another organization has** stays in that organization's Workforce.
- **An AI tool's update does not wait for other organizations' workers** already using the tool;
  it holds their new work only (§4). **Signing in to an AI tool** holds the first organization's
  new work only. Both are the AI tools page's own rules, which this phase leaves alone (ADR-090).
- **Settings Plenipo could not read** are checked in the first organization only (Phase 13's
  notice); another organization's damaged settings show on the pages that use them.
- **A page listening for every window's updates would hear another organization's.** Tauri sends
  an event meant for one window to any page that asks for every window's; Plenipo's pages always
  ask for their own window's only (§12), and a test holds them to it.
- **Restoring a backup restarts all of Plenipo** (§10).

## Consequences

- Each open organization costs its own set of services and a small tool server; a handful of
  organizations is the expected case.
- A second organization's secrets, sign-ins, and backups are separate by construction: another file,
  another folder, another name in the Vault.
- Commands keep their names and their permission files; only where they find their organization
  changed, so the pages did not.

## As built

Built on 2026-09-30 (v1.16.0) as decided, with the limits above. Where the build adds to it:

- **Code:** `orgs.rs` (the list, each open organization's services, which window shows which, and
  the `Org` command argument), `org_host.rs` (opening an organization), `org_commands.rs` (the
  commands), and `tool_holds.rs` (an AI tool's update) in the app; `copy_setup_from` in the Ledger
  (`workforce/copy.rs`); your Workforce and tile in `Workforce::share_with`; the shared record of
  who uses the screen in `Broker::sharing_control`; `apps/desktop/src/orgs/` on screen.
- **§12, how a command finds its organization:** every command that works on an organization's
  things takes `Org<…>` in place of `State<…>`, which reads the calling window's label. The PC's
  commands (the AI tools page, updates, start and close, notice choices, the terminal's shell, Stop
  all) keep the first organization's.
- **§12, what a page hears:** Plenipo sends each organization's events to its window only, and the
  page listens for its own window's events only (`listenHere` in `api/events.ts`).
- **§15, a copy** goes into the new, empty Ledger before any service starts on it, so every role,
  department, and position keeps its ID and every setting still names the same one. Positions that
  staff a project stay behind with it.
- **§18, deleting for good** saves each chosen worker into the first organization's Ledger with its
  role and specialty found by name; a folder a file still holds open is removed at the next start.
- **Each window's own place:** the first organization keeps the page's remembered names; another
  organization's add its ID (`plenipo.place@<id>`), so each remembers its own page, map, and panels.
- **Checked:** `two_organizations_in_two_windows_never_cross`,
  `an_organization_is_archived_brought_back_and_deleted_for_good`, and
  `a_new_organization_starts_from_scratch_a_copy_or_not_yet_a_template` (IPC);
  `each_organizations_backups_are_its_own`; the copy and shared-Workforce tests in
  `crates/workforce/tests/owner_control.rs`; the uninstaller's test; `Organizations.test.tsx`,
  `events.test.ts`; and `organizations.e2e.mjs` in the real app.

## Alternatives considered

- **One Ledger for every organization, with an organization ID on every row.** Rejected by
  ADR-039 §2.10 (one Ledger file per organization), and it would make a mistake in one query leak
  one organization's work into another.
- **One organization open at a time.** Rejected in ADR-091 §3: the plan's acceptance opens a
  client's organization beside the first.
- **A separate file for the PC's shared record now.** Deferred: it means moving your Workforce and
  tile out of a Ledger that its events already point at; the first organization keeps them, and is
  kept for good.
- **Moving the first organization into `organizations\` on update.** Rejected: moving a Ledger and
  its working copies on an update is a risk with nothing gained.
