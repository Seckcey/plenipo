# Phase 12 — Acceptance Report

|              |                                                                                                                                                                                                                                                                                                                                                                                  |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 12 — Product UX, Notifications, Settings, and Operator Experience                                                                                                                                                                                                                                                                                                                |
| **Branch**   | `claude/wizardly-wozniak-uupnb5` ([PR #65](https://github.com/Seckcey/plenipo/pull/65))                                                                                                                                                                                                                                                                                          |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, `pnpm bindings` (no diff), and the full `pnpm e2e` (73 tests) against the release build. GitHub CI: Rust, Frontend, E2E (Linux), Windows — see the pull request.                                                                                                                                                        |
| **Date**     | 2026-09-27                                                                                                                                                                                                                                                                                                                                                                       |
| **Result**   | Both acceptance criteria and all twelve Phase 12 tests pass; every deliverable is built. Version **1.8.0**. Decisions: ADR-031 (the terminal panel), accepted and built; ADR-033 (Home, a page for each thing, pop-up notices, and Settings in one place), proposed. Real Windows notices, and the terminal with PowerShell and a real server, are the owner's check on Windows. |

Screenshots (from the end-to-end run in the real app):

- **Home:** [a new company, with Pip waving hello](evidence/phase-12/home-empty.png) ·
  [a company at work, dark](evidence/phase-12/page-home-dark.png) ·
  [light](evidence/phase-12/page-home-light.png)
- **A page for each thing:** [a department](evidence/phase-12/page-department.png) ·
  [a project](evidence/phase-12/page-project.png) · [a worker](evidence/phase-12/page-worker.png)
  · [an objective (a task)](evidence/phase-12/page-task.png)
- **The terminal:** [nothing open yet](evidence/phase-12/terminal-empty.png) ·
  [this PC, running what you type](evidence/phase-12/terminal-this-pc.png) ·
  [on the right](evidence/phase-12/terminal-right.png) ·
  [on a server, signed in](evidence/phase-12/terminal-server.png) ·
  [a changed server ID refused](evidence/phase-12/terminal-id-changed.png) ·
  [a worker's watch tab](evidence/phase-12/terminal-watch.png)
- **Settings:** [Notifications](evidence/phase-12/settings-notifications.png) ·
  [Terminal](evidence/phase-12/settings-terminal.png) ·
  [Local paths](evidence/phase-12/settings-local-paths.png) ·
  [About Plenipo](evidence/phase-12/settings-about.png) ·
  [Servers, as before](evidence/phase-12/servers-settings.png)
- **The older pages, now from the library**, in both themes: Organization
  ([dark](evidence/phase-12/page-organization-dark.png),
  [light](evidence/phase-12/page-organization-light.png)), Projects
  ([dark](evidence/phase-12/page-projects-dark.png), [light](evidence/phase-12/page-projects-light.png)),
  Workers ([dark](evidence/phase-12/page-workers-dark.png),
  [light](evidence/phase-12/page-workers-light.png)), Approvals
  ([dark](evidence/phase-12/page-approvals-dark.png),
  [light](evidence/phase-12/page-approvals-light.png)), AI tools
  ([dark](evidence/phase-12/page-ai-tools-dark.png), [light](evidence/phase-12/page-ai-tools-light.png)),
  Activity ([dark](evidence/phase-12/page-activity-dark.png),
  [light](evidence/phase-12/page-activity-light.png)), Settings
  ([dark](evidence/phase-12/page-settings-dark.png), [light](evidence/phase-12/page-settings-light.png)),
  Diagnostics ([dark](evidence/phase-12/page-diagnostics-dark.png),
  [light](evidence/phase-12/page-diagnostics-light.png)); an
  [approval card](evidence/phase-12/approval-card.png) and the
  [control notice with Stop all](evidence/phase-12/control-banner.png).
- **The library:** [menus and tabs you can close](evidence/phase-12/gallery-dark-notices.png)
  ([light](evidence/phase-12/gallery-light-notices.png)) · the frame
  ([dark](evidence/phase-12/frame-dark.png), [light](evidence/phase-12/frame-light.png)) ·
  [the smallest window](evidence/phase-12/min-window.png).

Test totals: **814 Rust** · **526 frontend** (304 design system + 222 app) ·
**73 end-to-end** tests against the real release binary (section 6).

On screen the plan's words become plain ones ([word list](../design/vocabulary.md)): the plan's
"Agent View" is a **worker's page** (about a position), a coordinator is a **Supervisor**,
providers are **AI tools**, capability profiles are **permission sets**, "runtime/session" is the
**conversation**, artifacts are **screenshots**, blocked tasks are **What's stuck**, approvals
waiting are **Waiting for you**, acceptance criteria are **Done when**, and notifications are
**notices**. Quotes from the plan keep the plan's words.

CI has no AI tool accounts, and tests never touch the internet or a real server: the AI tools are
`plenipo-fake-agent`, and the server is `plenipo-test-sshd` on `127.0.0.1` (with a small shell for
the owner's terminal).

## 1. Acceptance criteria → evidence

| #   | Criterion (ROLLOUT_PLAN.md)                                                                                   | Result   | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| --- | ------------------------------------------------------------------------------------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | The normal user experience does not require reading terminal output, editing JSON, or memorizing session IDs. | **Pass** | Plenipo opens on **Home**, which says in a line how things are and lists what waits for the owner, what's stuck, each department's health, the objectives going, who's working, and what just finished; each opens its own page (E2E `opens Home and the pages of a department, project, worker, and task, with Back`). The pages show results as answers, states as plain labels, and history as plain lines: an AI tool's conversation number is kept to Diagnostics and the Activity trail (`historyLine`). Everything the owner chooses is a switch, a list, or a form in **Settings** (no JSON). Pop-up notices tell the owner when something needs them. |
| 2   | Raw diagnostics remain available for troubleshooting.                                                         | **Pass** | Settings → **Diagnostics** shows a summary and opens the Diagnostics page (the Ledger, its backups and exports, the programs, the AI tools, the raw details), which is unchanged; the Activity page keeps every event. Settings → **Local paths** shows where Plenipo keeps its files (`get_local_paths`). Screenshots: [Diagnostics](evidence/phase-12/page-diagnostics-dark.png), [Local paths](evidence/phase-12/settings-local-paths.png).                                                                                                                                                                                                                 |

## 2. Deliverables → evidence

| Plan deliverable                                                                                                                                                                                   | Built as                                                                                                                                                                                                                                                                                                                                                                                                                                               | Tests                                                                                                                                                                                                                                       |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Home / Company:** department health, current objectives, agents working, blocked tasks, approvals waiting, recent completions                                                                    | `pages/HomePage.tsx`: Pip's greeting and a line on how things are (his pose follows what matters most); tiles; **Waiting for you**, **What's stuck**, **Departments** (a card each: health, its Manager, its 24 hours), **Current objectives**, **Who's working**, **Just finished**. Workforce `home()` and Ledger `problems`, `open_objectives`, `finished_objectives`, `objective_counts`.                                                          | `pages.test.tsx` Home (7), Ledger `what_is_stuck_…`, `objectives_going_…`; Workforce `a_worker_that_fails_…` (Home); E2E design (pages), workforce (a new company).                                                                         |
| **Department View:** manager, projects, current workers, queue, performance/activity history                                                                                                       | `pages/DepartmentPage.tsx`: About (its Manager), Projects, Working now, Queue, Everyone, the last day and week, and its History a page at a time.                                                                                                                                                                                                                                                                                                      | `pages.test.tsx` department (2); Ledger `a_scopes_history_…`; E2E design.                                                                                                                                                                   |
| **Project View:** coordinator, repository/workspace, task tree, running workers, branches/PRs, artifacts, recent decisions                                                                         | `pages/ProjectPage.tsx`: About (its Supervisor, folder, repository), Working now, Objectives and the task tree of each, Branches and pull requests, Screenshots, Recent decisions, History.                                                                                                                                                                                                                                                            | `pages.test.tsx` project; Ledger `a_projects_record_…`; E2E design.                                                                                                                                                                         |
| **Agent View:** role, selected provider/model, current task, capabilities granted, runtime/session, event history                                                                                  | `pages/WorkerPage.tsx` (a position, ADR-033 §2): its role, AI tool and model and **why this AI tool**, Working on, **Permissions in use**, its **Conversation**, Recently finished, History.                                                                                                                                                                                                                                                           | `pages.test.tsx` worker; E2E design.                                                                                                                                                                                                        |
| **Task View:** objective, acceptance criteria, delegation tree, activity stream, artifacts, approvals, final result                                                                                | `pages/TaskPage.tsx`: the objective and **Done when**, Given to, Part of, the delegation tree, Approvals, the **Result** (an objective's whole result; a task's links to it), Screenshots and pull requests, Decisions, and its Activity as it happens.                                                                                                                                                                                                | `pages.test.tsx` task (4); Ledger `a_task_tree_has_its_events_…`; E2E design.                                                                                                                                                               |
| **Terminal panel** (ADR-031, accepted)                                                                                                                                                             | `apps/desktop/src/terminal/`, `crates/capabilities/src/terminal.rs`, `broker/terminals.rs`: at the bottom or on the right, resizable, remembered; **Terminal** button and **Ctrl+\`**; this PC (Windows PowerShell by default; never as administrator) and the owner's servers (pinned ID checked first; Remote computers (SSH) on); watch tabs with **Stop** and **Disconnect**; only opening and closing recorded. See the checklist for every item. | `TerminalPanel.test.tsx` (9), `watch.test.ts` (10); Rust `tests/terminal.rs` (9), `tests/ssh.rs` (the owner's Stop; a worker cannot type into the terminal), IPC `the_terminal_is_the_owners_alone`; E2E terminal (3), servers (watch tab). |
| **Settings:** providers, authentication state, role/model policies, fallback order, capability profiles, projects, departments, approval rules, local paths, notification preferences, diagnostics | `views/SettingsView.tsx` and `settings/`: sections on the left, one at a time, the last one comes back: AI tools (each tool and its sign-in), AI models (role choices, first choice and backups in order), Permissions (permission sets and approval rules), Organization (departments and projects), Servers (unchanged), Switches, **Notifications**, **Terminal**, Personalization, **Local paths**, Diagnostics, **About Plenipo**.                | `settings.test.tsx` (7); E2E design (Settings), and every earlier spec now opens its section (`openSettings`).                                                                                                                              |
| **Notifications** (the approved plan: Windows pop-up notices, and a place to choose them)                                                                                                          | `crates/ledger/src/notices.rs` (what each event means for the owner, the choices, gathering and no repeats within a minute) and `src-tauri/src/notices.rs` (the listener and its thread, the plugin); Settings → Notifications with **Send a test notice**. The plugin's own commands are granted to no window.                                                                                                                                        | Ledger notices (6), desktop notices (3), IPC `notices_and_local_paths_are_the_main_windows_alone`, `settings.test.tsx`; E2E design (the test notice).                                                                                       |
| **Existing screens** rebuilt from the library (ADR-030 §8)                                                                                                                                         | Buttons, status pills and plain tags, tabs, and cards on the older pages come from `@plenipo/ui`; `styles.css` keeps page layout only. What each page does is unchanged: every earlier end-to-end test passes (section 6).                                                                                                                                                                                                                             | The earlier specs, unchanged in what they check; the Gallery look snapshot.                                                                                                                                                                 |
| **The owner's brand** (owner request)                                                                                                                                                              | The Plenipo + Pip kit kept byte for byte in `docs/brand/pip-brand-kit`; the logo and app icon; `PlenipoMark`, `PlenipoLogo`, `Pip` (15 poses) in the library; Pip on Home, in empty and error states, in the terminal panel, and in Settings → About.                                                                                                                                                                                                  | `brand.test.tsx`; E2E design and workforce (Pip is drawn on Home and in About).                                                                                                                                                             |

## 3. Plan tests → evidence

| Plan test                                                                                           | Test                                                                                                                                                                                                                                                                                                                                 | What it shows                                                                                                                                                          |
| --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| keyboard navigation                                                                                 | E2E design `works from the keyboard…` (Home first in the Tab order), terminal (resize from the keyboard; Ctrl+\`); `TerminalPanel.test.tsx` (F6 leaves the terminal for its tabs; the arrow keys move along the tabs; Hide and Ctrl+\` give the keyboard back to the Terminal button); `menu.test.tsx` (Escape and Tab close a menu) | Every page is reached from the strip in order; the terminal's edge moves with the arrow keys; Tab stays the shell's, and F6 and Ctrl+\` get out.                       |
| state restoration                                                                                   | E2E design (the page comes back after a restart, and Back then goes to its section), terminal (the panel's place and size after a restart); `App.test.tsx`; `settings.test.tsx` (the last section comes back)                                                                                                                        | The page you were on, the panel, and the Settings section come back.                                                                                                   |
| large task history                                                                                  | Ledger `a_large_history_opens_a_page_at_a_time_quickly` (33,000 events); `pages.test.tsx` `keeps every event after Show older…`                                                                                                                                                                                                      | A task tree and a quiet department's whole history, page by page, every event once and in order, each page well under a second; What's stuck reads only the last week. |
| large org tree                                                                                      | `pages.test.tsx` `stays usable with a large organization`; `org/layout.test.ts` (20 departments × 5 projects × 10 workers)                                                                                                                                                                                                           | A card per department and everyone working listed, quickly; the map lays out a large organization.                                                                     |
| disconnected providers                                                                              | `pages.test.tsx` (a position that can't work is stuck on Home, and its department's health says so); Ledger notices (an AI tool signed out or at its usage limit makes a notice); `settings.test.tsx` (AI tools and their sign-in)                                                                                                   | An AI tool that is signed out or unavailable shows where the owner looks first.                                                                                        |
| empty states                                                                                        | `pages.test.tsx` (a new company), workforce E2E (Pip waving hello), `TerminalPanel.test.tsx` (no terminal open)                                                                                                                                                                                                                      | Every list says so when it has nothing, with the next step.                                                                                                            |
| error states                                                                                        | `pages.test.tsx` (What's stuck and the requests couldn't be read: never "nothing"; a task that can't be read, and one that isn't in the Ledger), `settings.test.tsx`, `TerminalPanel.test.tsx` (a terminal that can't open; a refusal shown in the terminal)                                                                         | Each says what failed, with **Try again**; nothing that failed is shown as "all clear".                                                                                |
| accessibility smoke tests                                                                           | `pages.test.tsx` and `settings.test.tsx` a11y smoke (`test/a11y.ts`: every control named, one main heading, no skipped level, named regions, unique ids, references that point somewhere)                                                                                                                                            | Every page and every Settings section passes.                                                                                                                          |
| terminal panel: open, hide, resize, and restore after a restart                                     | E2E terminal `opens from the top bar…` and `comes back after a restart…`                                                                                                                                                                                                                                                             | In the real app, with the owner's closing recorded ("Plenipo closed").                                                                                                 |
| the owner's terminal on this PC and on a synthetic SSH server, with a changed server ID refused     | Rust `plan_the_owners_terminal_on_this_pc`, `plan_the_owners_terminal_on_a_server`, `plan_a_changed_server_id_is_refused`; E2E terminal `opens a terminal on a server…`                                                                                                                                                              | What is typed reaches the shell and nothing typed or shown is recorded; a server that shows another ID is refused before Plenipo signs in.                             |
| a worker's watch tab shows its commands and output live, and Stop and Disconnect there end its work | `TerminalPanel.test.tsx` `opens a watch tab by itself…`; Rust `the_owners_stop_ends_only_the_command_running_now`; E2E servers (the watch tab)                                                                                                                                                                                       | The tab opens by itself, shows each command and its output with secrets hidden, and its Stop ends the command running now.                                             |
| a worker cannot send keystrokes to the owner's terminal                                             | Rust `plan_a_worker_cannot_type_into_the_owners_terminal` (the tool relay refuses the names), IPC `the_terminal_is_the_owners_alone` (the sign window and web pages are refused), `computer_use_asks_first_and_never_types_secrets` (while a worker has the screen, the terminal takes nothing)                                      | No tool, window, or page but the owner's main window reaches the terminal, and a worker using the mouse and keyboard can't type into it.                               |

## 4. The owner's decisions → evidence

| Decision (2026-09-27)                                                                                 | Evidence                                                                                                                                                     |
| ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| The Phase 12 plan is approved (Home, a page for each thing, notices, Settings in one place, terminal) | Sections 1 and 2.                                                                                                                                            |
| ADR-031 (the terminal panel), accepted with its choices                                               | Built as written; see the checklist's terminal items.                                                                                                        |
| Keep the Plenipo + Pip brand kit in the repository, brand the app, and make Pip prominent             | `docs/brand/pip-brand-kit` (byte for byte, with its manifest); the logo and app icon; Pip on Home, in empty and error states, in the terminal, and in About. |
| Settings → Servers keeps working as before                                                            | The same component in its own section; the Phase 11 end-to-end test passes unchanged in what it checks.                                                      |
| Version 1.8.0                                                                                         | Every manifest, `Cargo.lock`, and `scripts/check-versions.mjs`.                                                                                              |

## 5. Defects found and fixed during Phase 12

In the end-to-end run:

- **Pictures in screenshots:** a screenshot taken right after a page opened could show an empty
  space where Pip goes (the test display draws a large picture a moment later). Screenshots now
  wait for pictures to be drawn, and the tests check that Pip has loaded.
- **Typing in the test display:** WebDriver's keys sent all at once dropped repeated keys and
  spaces in the terminal; the test types one key at a time, as a person does.
- **The New terminal list** did not change while the panel was open when a server was added or
  Remote computers (SSH) switched on. It now follows Settings.
- **Quitting** could finish before a terminal's closing was written; the closing is now written
  first, and Plenipo waits for a server's terminal to answer.

In GitHub's checks on Windows:

- **Three terminal tests were refused** on GitHub's Windows machines, which run everything as
  administrator: Plenipo rightly opens no terminal then. The tests now turn that refusal off to
  reach PowerShell there (Plenipo itself never does), and a new test checks the refusal.

A code review in five areas then ran on the finished work, and a second reviewer checked each
finding before it was fixed. Fixed:

- **What's stuck** read every event in the Ledger on each Home refresh, could miss older
  problems in a busy week, counted a task waiting its turn as stuck, kept a server whose ID was
  set right, and counted Diagnostics' test tasks. It now reads the last week only (through the
  time index), looks only at real problems, follows Settings → Servers' rule for a server, and
  counts only the company's work. **Just finished** is in the order work finished, and the
  counts on Home are whole, not a page.
- **History** after **Show older** could skip events when many arrived at once; it now reads the
  events in between. A scope's history reads the newest events in order first, so a busy
  department's page no longer sorts its whole history.
- **Home** said "Nothing is waiting" or "Nothing stuck" when it couldn't read them; it now says it
  couldn't check, with Try again.
- **The task page** read all of a task's events after every change; it now reads its tree, and a
  task's page no longer builds its objective's whole result. A task that isn't in the Ledger
  says so, with **Go to Home**.
- **The terminal:** a reload of the window left terminals running unseen (they now close with
  the page, and one still connecting closes as it opens); turning Remote computers (SSH) off
  left server terminals open (they now close);
  closing the window quit Plenipo and cut a running terminal (it now hides to the tray, as
  running work does); the open limit did not count terminals still connecting; a worker using
  the screen, mouse, and keyboard could type into the terminal (the terminal now takes nothing
  until the owner takes over, and a new line or Ctrl+J asks the owner, as Enter does); a paste
  over 64 KiB was lost without a word (it is sent in pieces, and a refusal is shown); a size
  change while connecting was lost; the keyboard could not leave the terminal (F6), a tab
  chosen with the arrow keys pulled the keyboard into its terminal, and closing the last tab
  or hiding the panel with Ctrl+\` dropped the keyboard at the top of the window; text on
  colored backgrounds was hard to read (a minimum contrast); a screen reader heard nothing (a
  switch in Settings → Terminal); a watch tab now says when output beyond 2,000 lines was not
  kept, and rebuilds after a reload without losing or doubling lines.
- **Words and looks:** refusals and usage limits on the older pages had the "waiting" look (now
  a warning, as before); plain names had a status mark (now a `Tag`); a disabled **Stop all**
  looked clickable; library panel titles took the older pages' heading size; "Removed" is
  **Inactive** for a department; "Awaiting approval" is **Waiting for you**; a handoff waiting
  its turn has plain words; two hints promised things that aren't so.
- **Settings:** a section opened from another page came back after Back or a restart instead of
  the one chosen; a quick second change to the notices could undo the first; Settings → AI tools
  kept "looking" when the tools couldn't be read.
- **Local paths** said nothing was kept when only the Ledger was temporary; the notices thread
  kept its Ledger alive.

## 6. Test totals

- **814 Rust** tests (Linux, after merging main), including the terminal's (9), the page queries
  (6), the notices (9), the browser tests against a real Chromium (18), and the IPC checks that
  the new commands answer the main window only and refuse the sign window and web pages.
- **526 frontend** tests: **304** in the design system and **222** in the app.
- **73 end-to-end** tests against the real release binary (Linux, after merging main), in 13
  groups, all passing, including **3 terminal** tests and **3 new page and Settings tests** in
  the design group.

## 7. Notes

- **Checked on Windows by the owner:** a real notice (Settings → Notifications → **Send a test
  notice**), and the terminal with PowerShell on this PC and on a real server. Windows builds and
  runs the tests in CI, but a real notice needs the installed app.
- **ADR-033 (Home, a page for each thing, pop-up notices, and Settings in one place) is
  proposed.** Accepting it means agreeing with how this phase put the plan together: the worker
  page is about a position, **Showing** opens a page, notices are decided by Plenipo and never by
  the page, local paths are shown, not changed, and the terminal's few extra rules (ADR-033 §8).
- **Still open from earlier phases:** ADR-025 (servers over SSH, through Guard) is proposed, and
  the owner's Phase 11 check on Windows with a real server is still to do.
