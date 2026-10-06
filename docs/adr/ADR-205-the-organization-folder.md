# ADR-205: The organization folder — one folder per organization, with its departments, projects, and scratch pads

- **Status:** Proposed. The owner's request (I3, 2026-10-05) and the owner's answers to the
  design (2026-10-05): every recommendation approved, with one change (§3: the folder stays in
  Documents even when OneDrive syncs it, with an alert to keep it on this device).
- **Date:** 2026-10-05
- **Phase:** 25
- **Amends:** [ADR-201 (light by default)](ADR-201-light-by-default.md) §11–13 (Plenipo's own
  folder becomes the agent's scratch pad, in a known place; built in part 3);
  [ADR-093 (your files and the editor)](ADR-093-your-files-and-the-editor.md) §1 and §23–24
  (Files gains the organization folder; part 2);
  [ADR-094 (more than one organization)](ADR-094-more-than-one-organization.md) §1 and §18 (each
  organization's own folder in Documents; Delete for good leaves it)
- **Keeps:** workers' file access goes through Guard, one folder per step
  ([ADR-013 (Guard and the capability broker)](ADR-013-guard-capability-broker.md) §6); working
  copies stay in Plenipo's data folder
  ([ADR-016 (the Development department)](ADR-016-development-department.md) §2)
- **Goes with:** ADR-206 (keeping the organization folder tidy, and its Guard rules)

> **On screen** ([ADR-010 (plain words and rank names)](ADR-010-plain-titles.md),
> `docs/design/vocabulary.md`): **organization folder**, **Files** (finished files), **Scratch
> pads**, **scratch pad**, **Change…**, **Show in folder**, **Always keep on this device**. Not
> "artifacts", "workspace", "sandbox", "data directory", or "root".

## In short

Each organization gets a folder of its own, `Documents\Plenipo\<organization>`, which you can see
and open. Inside it, Plenipo keeps a folder for each department, each department's projects
inside it, a **Files** folder for each department's and project's finished documents, and a
**Scratch pads** folder with one scratch pad for each agent. Plenipo makes these itself, not an
AI, so the shape is always right, and records each one so it never moves on its own. When OneDrive
syncs your Documents folder, the organization folder still goes there, and Plenipo tells you, in
plain words, to set it to **Always keep on this device**. **Accepting this record means** building
it as written below, in four parts; part 1 is built (As built).

## Context

The owner (I3, 2026-10-05): "When creating an organization, we need to setup an organization
folder that will hold all the departments, projects, and agent scratch pads. That org folder will
be what comes up in the files tab section. Right now it doesn't assign any storage for artifacts
or anything. Each supervisor is responsible for keeping the org file structure in order and
organized."

What was there (read at `306af756`, v1.26.0):

- An organization is a Ledger in Plenipo's hidden data folder, and a line in
  `organizations.json`. No folder of the owner's is stored for it.
- Work with no project folder goes to `<Documents>\Plenipo\<organization>\<project or position>`
  (ADR-201 §11), worked out from the organization's **name** at the moment of use
  (`Broker::own_folder`). Nothing records it: renaming the organization sends new work elsewhere,
  two organizations with the same name share one folder, and Files can't show it.
- Files shows only project folders typed by hand, and working copies (ADR-093).
- Projects' folders are typed by hand, with no folder chooser in Plenipo.

## Decision

### 1. Where the organization folder lives

1. **By default, `<Documents>\Plenipo\<organization>`**, where ADR-201's folders already are.
   Where the PC has no Documents folder, Plenipo's data folder's `files`.
2. **The owner can choose another place** with **Change…**, which opens the system's own folder
   chooser over the window. A chosen folder that is empty becomes the organization folder; one
   with things in it holds the organization folder, named after the organization.
3. **Places that can never be one** (Guard's `places`), refused with the reason in plain words:
   Plenipo's data folder or anything inside it; Windows, `Program Files`, `Program Files (x86)`,
   `ProgramData`, and anything inside them (on a Mac or Linux, `/etc`, `/usr`, `/System`, and the
   like); a Startup folder; the owner's own top folder (`C:\Users\<you>`, and `C:\Users`); a
   drive's or a network share's top folder; a path that isn't full, or has `.` or `..`; and any
   path with a **junction or link** on the way, below the owner's own top folder and the Documents
   folder (which are the system's). OneDrive's cloud files are reparse points too, but they are
   the file itself, not a pointer elsewhere, so they are not refused (ADR-214 lets them through
   on purpose too).
4. **The name** is the organization's, cleaned to one ordinary folder name (no `<>:"/\|?*`, no
   dot or space at either end, no device name, at most 60 characters). A place already taken (by
   a file, a link, or a folder with things in it) gets "(2)", and so on. An empty folder there is
   used as it is.
5. **Written down once** (§2): renaming the organization never moves or renames its folder.

### 2. What goes inside, and what is recorded

6. **The shape**, made by Plenipo as the org chart changes:

   ```text
   <organization>\
     Read me.md                  what this folder is (made by Plenipo, from 8 West Ventures, LLC)
     <department>\
       Files\                    the department's finished documents
       Scratch pads\<agent>\     its head's, and its agents' on no project
       <project>\
         Files\                  the project's finished documents
         Scratch pads\<agent>\   its Supervisor's and its team's
         <repository>\           its code, when Plenipo makes a GitHub copy (ADR-204)
     Scratch pads\<agent>\       agents in no department
   ```

   A position's department and project come from its reporting line: the nearest department head
   and project Supervisor at or above it.

7. **The Ledger's `folders` table** (schema version 14) records each folder once: its kind
   (`organization`, `department`, `department_files`, `project`, `project_files`, `scratch_pads`,
   `scratch_pad`), what it belongs to, its path, and whether Plenipo made it or found it empty
   there. Rows are never deleted. Events: `folder.made`, `folder.adopted`, `folder.remade`; and,
   in later parts, `folder.moved` (the owner's click) and `folder.tidied` (ADR-206).
8. **A recorded folder that has gone missing is made again at its recorded path**, with
   `folder.remade`, never somewhere else. One that became a junction or link is left alone and
   reported. Nothing is ever made through a junction.
9. **Plenipo's private things stay in its data folder:** the Ledger and its backups, screenshots,
   the browser profile, attachments, tool tickets, and working copies (ADR-016). Nothing secret
   goes in the organization folder.

### 3. OneDrive and other sync services (the owner's change, 2026-10-05)

10. **The folder stays in Documents even when OneDrive syncs Documents.** Instead, Plenipo shows an
    alert, word for word: "Your organization folder is in OneDrive. OneDrive can keep files only
    online, and your workers can't read those until they download. In File Explorer, right-click
    the **<organization>** folder and choose **Always keep on this device**."
11. **How Plenipo tells:** the folder is inside the path in `OneDrive`, `OneDriveCommercial`, or
    `OneDriveConsumer`; it is kept on this device when the nearest folder at or above it is marked
    pinned (`FILE_ATTRIBUTE_PINNED`), not when it is marked unpinned.
12. **Where it shows:** when the folder is made (the New organization dialog), on the
    organization folder's row in Settings → Organization, and in Files while it isn't pinned
    (part 2). **Show in folder** and **Check again** go with it.
13. **Where Plenipo can't tell** (Dropbox, Google Drive, iCloud): the alert shows when the folder
    is made and stays on the folder's row, in words for that case (on a Mac, **Keep Downloaded**).
14. **Files kept only online** are handled on purpose (ADR-206): the tidy-up never reads one, a
    move is a rename that downloads nothing, and Files marks them "online only".

### 4. Files, existing organizations, and the rest (parts 2 to 4)

15. **Files opens the organization folder** (a new top folder, `org:`), with Plenipo's folders
    marked, working copies under their project, and **Elsewhere on this PC** for project folders
    outside it. ADR-093's rules stay (one writer, blocked files, never starting a program).
16. **Work goes in its place** (part 3): work with no project, and a lead's own work, in its
    scratch pad (ADR-201's own folder, now recorded); a project with no code in its `Files`;
    every agent told where its scratch pad is and where finished work goes.
17. **Organizations made before this** (part 4) are offered a folder; nothing moves by itself.
    Making it adopts `Documents\Plenipo\<organization>` when it is already there, records each old
    folder where it is, and offers **Move into place**, one click each, only when no worker is
    using it. Project folders the owner chose stay where they are.
18. **Archive** leaves the folder alone. **Delete for good** leaves it on the PC, like project
    folders today, and says so.
19. **Backups:** Plenipo backs up its record every day, as before. The organization folder is
    ordinary files, covered by the owner's usual backup; Settings says so in plain words.
20. **Free and Pro both** get the organization folder.

### 5. What can act inside the organization folder (the reviewer's O3)

21. **Every step that can write or run programs gets a leaf folder** (a scratch pad, a `Files`
    folder, a working copy, or a code folder), never the organization folder, a department's, or
    a project's own folder. The only exception is ADR-206's tidy-up, which can only move files.
22. **No AI tool's own file tools write** in any step: Claude Code runs with none of its own tools,
    Codex with its shell off and its read-only sandbox (ADR-051), Kimi's file reads and writes
    come through Plenipo (ADR-027), and Copilot, Antigravity, paid keys, and Ollama can't change
    files. Grok's own profile is checked before its steps get a folder here (part 3).
23. **Programs and scripts** a step may run (under its permissions; Everyday work allows them
    under Light) run as the owner (ADR-034). **Their protection is the command and script checks,
    not the path checker:** deleting, moving, or overwriting outside the folder asks; the Never run
    list; scripts follow the same lists (ADR-214); Careful and Strict ask or refuse more. Nothing
    here loosens them.

## Consequences

- The owner has one folder to look in for everything an organization makes, and Files shows it.
- The folder's shape can't drift: Plenipo makes it, records it, and makes a missing piece again in
  the same place.
- One new table and one schema version (14). An older Plenipo refuses the newer Ledger, as before
  (ADR-006).
- One new library, `rfd` (MIT), the system's own folder chooser, opened from Rust only.
- Paths can get long in a department's project's code folder; a short organization folder
  (`C:\Work\Acme`) keeps them short.
- A department's head can read the scratch pads of every project in its department (ADR-206).

## Alternatives considered

- **Move the folder out of Documents when OneDrive syncs it** (to `C:\Users\<you>\Plenipo`).
  Recommended in the design; **not chosen by the owner**: keep Documents, tell the owner to keep
  the folder on this device.
- **Projects side by side at the top** (`Departments\`, `Projects\`, `Scratch pads\`): shorter
  paths and nothing to move when a project changes department. Not chosen: the owner described
  departments holding their projects, and each lead's area is one folder.
- **Working copies in the organization folder.** Not chosen: they are temporary, full copies of the
  code for each objective, and git's own business (ADR-016).
- **Let an AI make the folders.** Not chosen: the shape would drift. Plenipo makes and records
  them; ADR-206's tidy-up only moves files inside them.
- **The dialog plugin for the folder chooser** (`tauri-plugin-dialog`). Not chosen: it moved Tauri
  itself and about 45 other packages to new versions; `rfd`, the chooser the plugin uses, adds one
  package and nothing else.

## As built

### Part 1 (B1): the folder exists (2026-10-05)

- **Code:**
  - the Ledger: `crates/ledger/migrations/0014_folders.*.sql`, `crates/ledger/src/folders.rs`
    (`Folder`, `FolderKind`, `record_folder`, `record_folder_remade`), in the export
  - Guard: `crates/guard/src/places.rs` (`folder_name`, moved from the broker unchanged;
    `place_problem`; `link_on_the_way`; `synced_by`; `kept_on_this_device`)
  - capabilities: `crates/capabilities/src/org_folder.rs` (`create`, `suggest`, `keep`, and
    `FolderKeeper`, which runs `keep` in the background after each change to the org chart, and
    when the organization opens)
  - the app: `folder_commands.rs` (`suggest_org_folder`, `choose_folder`, `get_org_folder`,
    `open_org_folder`, an organization's window's alone); `create_organization` takes `folder`;
    `OrgStack.folders`
  - on screen: `orgs/NewOrganizationDialog.tsx` (the folder line, **Change…**, the alert),
    `orgs/OrgFolder.tsx` (the alert, and Settings → Organization's **Organization folder** row)
- **§1.2, a chosen folder:** checked again when the organization is made, before anything is made.
- **§2, Scratch pads** of agents in no department: made the first time one is needed.
- **Not yet:** Files (part 2, below), work in its place (part 3), and organizations made before
  this (part 4). The first organization, and every organization made before this version, show
  "This organization has no organization folder yet."
- **Checked:**
  - the Ledger's `folders` tests (one of each kind, the organization folder first, paths, events,
    rows never deleted)
  - Guard's `places` tests (names; the system's places; a real junction on Windows, a link
    elsewhere, refused whether it leads out or back in; who syncs a folder; the pinned mark)
  - `org_folder` tests (the shape follows the org chart, names and "(2)", the owner's folder left
    alone, a missing folder made again where it was, nothing made through a junction, the keeper
    in the background)
  - IPC: `the_organization_folder_commands_are_an_organization_windows_alone`,
    `a_new_organization_gets_its_organization_folder`
  - the screens: `Organizations.test.tsx` (the folder line, Change…, a refused place, the OneDrive
    alert word for word, the Settings row, another sync service's words)
- **The page names the folder** (`create_organization`'s and `suggest_org_folder`'s `folder` are
  free text from the window): that is the approved design, and Guard's `place_problem` checks the
  text before anything is made (the reviewer's N4 on #221).

#### The independent review's fixes (#221, 2026-10-05)

- **S1, this PC through a network name:** `\\localhost\C$\…`, `\\127.0.0.1\…`, `\\[::1]\…`, this
  PC's name or DNS name, and WSL's `\\wsl.localhost\…` and `\\wsl$\…` are refused, written or as
  the path really is (a mapped drive): "That's this PC's own drive through a network name. Choose
  it by its drive letter, such as C:\Work\Acme." A whole drive's hidden share (`C$`, `ADMIN$`) on
  any PC is refused too, which also covers this PC by its network address. A shared folder on
  another PC stays allowed.
- **S2, compared part by part:** every check compares paths name by name (`parts`, `same_place`,
  `within`), so `\\?\C:\` is `C:\`, `\\?\UNC\server\share` is `\\server\share`, `/` is `\`, and
  letter case is ignored where the system ignores it. A long path (which comes back as `\\?\C:\…`)
  or one written with `/` no longer slips past. `\\?/` and `\\./` mixes are refused like `\\?\`.
- **S3, never a level up:** a project whose department's folder can't be had this pass, and an
  agent whose team's **Scratch pads** can't, wait for the next pass and are never made (and
  recorded for good) a level up. A recorded folder that is now a junction, a file, or behind one is
  not used as a parent. Only a project in no department goes in the organization folder itself.
- **N1:** a network share is kept as `\\server\share\…`, which File Explorer opens.
- **N2:** an agent's own folder (work with no project) that is a junction or link is not used.
- **N3:** a folder Plenipo isn't allowed to look into on the way, a file, and a drive this PC
  hasn't are refused in plain words, not passed or left to fail later.
- **Known follow-up:** Guard's confinement (`crates/guard/src/paths.rs`) compares with
  `Path::starts_with`, which a long path's `\\?\` form could make miss a match the same way. It is
  outside the organization folder's checks and is left for its own change.

### Part 2 (B2): Files opens the organization folder (2026-10-05)

- **Code:** `crates/capabilities/src/broker/owner_files.rs` (the `org:folder` top folder, its
  marks, and the one-writer check); `FileRootKind::OrganizationFolder`, `FileRoots.organization`,
  `FileRoot.insideOrganization`, `FolderEntry.place` and `onlineOnly`, `FolderPlace`;
  `plenipo_guard::places::online_only`; on screen, `files/FilesPanel.tsx` and
  `ledger/format.ts`.
- **§15, what Files shows:** the organization folder first, opened by itself, with its folders
  marked (Department, Project, Finished files, Scratch pads, "Website Supervisor's scratch pad");
  then **Working copies**, for projects whose folder is inside it; then **Elsewhere on this PC**,
  for project folders outside it, each with its working copies, as before. An organization with no
  organization folder shows Files exactly as before; with nothing at all, "No organization folder
  yet".
- **§12, the alert in Files:** at the top while OneDrive keeps the folder online and it isn't set
  to stay on this device, with **Show in folder** and **Check again**.
- **§14, files kept only online:** marked "online only" from the folder listing's own marks
  (`FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`, `RECALL_ON_OPEN`, `OFFLINE`), so looking never
  downloads them.
- **One writer at a time (ADR-093 §10) now reaches every folder a step may change files in**,
  through whichever top folder it is reached: a working copy, a project folder, and, new, an
  agent's own folder. Before, a step in its own folder was not counted, so the owner could save
  over a file a worker was writing there.
- **A change a worker makes** in a project folder inside the organization folder is marked where
  the organization folder shows it too. A change in an agent's own folder is not marked yet: Watch
  doesn't name its folder (part 3).
- **A save in the organization folder** is recorded as `file.saved` with `place:
"organizationFolder"` and no project; Activity says "You saved … in the organization folder".
- **An organization folder that became a junction or link** isn't opened, and Files says it isn't
  there.
- **Checked:**
  - `owner_files` tests (the first folder with its marks, projects inside it, reading and saving
    in it and nowhere outside, `org:` names, a folder that became a junction)
  - the broker's `an_agents_own_folder_is_read_only_for_the_owner_while_it_works_there`
  - Guard's `files_kept_only_online_are_known_by_their_marks`
  - `Files.test.tsx` (opened first, marks, online only, Working copies, Elsewhere on this PC, the
    alert, the empty state) and `format.test.ts`

#### The independent review's fixes (#224, 2026-10-05)

- **S1, never opened to be listed:** a file kept only online is described from the folder
  listing's own marks alone. Following it as a link would open it, and OneDrive downloads a file
  marked to download when opened (`RECALL_ON_OPEN`). A cloud file is never a link, so nothing is
  lost. Opening it in Plenipo still downloads it, as the page says.
- **N1:** the one-writer check and the marks compare paths part by part, with Guard's
  `same_place` and `within` (the same as #221's S2), so a long path can't miss its writer.
- **N2:** the owner can't save a file where one of the organization's folders belongs, even while
  that folder is missing.
- **N3, for later:** the file view looks up where each recorded folder really is each time it
  lists. If Files feels slow on a large organization folder, keep that and refresh it on the
  `folder.*` events.

### Part 3 (B3): work goes in its place (2026-10-05)

- **Code:** `crates/capabilities/src/broker.rs` (`organization_place`, `project_files_folder`,
  `own_folder`, `try_open`, and the tools note), `broker/owner_files.rs` (`marked`),
  `plenipo_guard::places::relative_parts`; the app passes the system's own folders
  (`BrokerConfig.trusted_places`); on screen, `components/org/ProjectFolderField.tsx` in New
  project, Edit project, and Set up a Development project.
- **§2.4, which folder each step gets,** with an organization folder:
  - **Work that belongs to no project** (a lead's own work too) is done in the worker's **scratch
    pad**, as recorded. Not recorded yet, or missing: one pass of keeping the folder makes it, at
    its place, before the step opens. Never through a junction, a link, or a folder Plenipo may not
    look into (then the step has no folder, and its note says so). A step that is no position's
    gets no scratch pad.
  - **A project with no folder of its own** works in its **Files** folder: that is its project
    folder. Its workers write there; its leads read there and hand changes on (ADR-016), as in any
    project folder. Before this, a lead wrote in Plenipo's folder for such a project; now its own
    notes belong in its scratch pad on work that is no project's. A Files folder is never a working
    copy's repository, even inside a git repository.
  - Code work is unchanged: the objective's working copy.
  - An organization without an organization folder works as before (ADR-201).
- **The tools note says where files go:** "Your scratch pad is …: … Keep your notes and drafts
  there, and save the files you make for the owner there."; "The project folder is …, the Website
  project's Files folder in the organization folder. Finished work for the project goes here, where
  the owner looks for it."; and on a working copy, "Your work is the project's code: put documents
  where the project keeps them (for example docs/)."
- **Files marks** a file a worker is changing in its scratch pad, or in a project's Files folder,
  where the organization folder shows it (the gap left by part 2).
- **The project dialog, "Where its files go":** **Make a folder in the organization folder** (the
  default for a new project: it works in its Files folder) or **Use a folder I already have**,
  written or picked with **Choose…** (the system's folder chooser; a place Guard refuses is said in
  plain words and not taken). An organization without an organization folder shows the folder box
  as before. **No folder**, the design's third choice, is left out: every project in an
  organization folder gets its Files folder anyway, so it would mean the same as the first.
- **Grok, checked:** Plenipo runs Grok with none of its own tools (its profile allows only the two
  that reach Plenipo's tool server, and names each of its own as disallowed), and refuses every
  other tool request (`crates/runtime/src/agent/grok.rs`). So, like every AI tool here, it changes
  files in the organization folder only through Plenipo's checked tools.
- **Checked:** the broker's `work_with_no_project_lands_in_its_scratch_pad`,
  `a_project_without_a_folder_works_in_its_files_folder`,
  `an_agents_own_folder_is_read_only_for_the_owner_while_it_works_there`, and
  `an_agents_own_folder_that_is_a_junction_is_not_used`; `the_note_says_where_files_go`;
  `changes_in_a_scratch_pad_and_a_files_folder_are_marked_in_the_organization_folder`; Guard's
  `the_names_below_a_place`; and `ProjectFolderField.test.tsx`.
