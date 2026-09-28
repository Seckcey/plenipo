# Phase 19 — Acceptance Report

|              |                                                                                                                                                                                                                                                                                                                                           |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 19 — The AI Tools Page: Sign-in, Usage, and Updates                                                                                                                                                                                                                                                                                       |
| **Branch**   | `claude/phase-19` ([PR #94](https://github.com/Seckcey/plenipo/pull/94))                                                                                                                                                                                                                                                                  |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, `pnpm bindings` (no diff), and the end-to-end tests that touch the AI tools page against the release build (section 3). GitHub CI on PR #94: see the pull request's checks (Rust, Frontend, Docs, Website, E2E on Linux, and Windows).                                           |
| **Date**     | 2026-09-28 (Pacific time)                                                                                                                                                                                                                                                                                                                 |
| **Result**   | Every acceptance criterion and every Phase 19 test in the plan pass; every deliverable is built. Version **1.12.0**. Decisions: ADR-058 to ADR-060, accepted by the owner with every choice as recommended; all built, with the details each records as built. The walk-through with real AI tools on Windows is the owner's (section 7). |

Screenshots (from the end-to-end run in the real app, `tests/e2e/specs/ai-tools.e2e.mjs`):

- **The page:** [each AI tool's card](evidence/phase-19/ai-tools-page.png)
- **Signing in:** [Codex's sign-in tab, running `codex login`](evidence/phase-19/ai-tools-sign-in-tab.png) ·
  [signed in again, checked by itself](evidence/phase-19/ai-tools-signed-in-again.png)
- **Usage and plan:** [what is left of Claude Code's plan](evidence/phase-19/ai-tools-plan-left.png) ·
  [this week's usage by model](evidence/phase-19/ai-tools-usage.png) ·
  [a usage limit, with Try again now](evidence/phase-19/ai-tools-usage-limit.png)
- **Updates:** [a new version of Grok](evidence/phase-19/ai-tools-update-ready.png) ·
  [updated with one click](evidence/phase-19/ai-tools-updated.png) ·
  [waiting while a task uses Grok](evidence/phase-19/ai-tools-update-waiting.png) ·
  [a failed update, the old version still working](evidence/phase-19/ai-tools-update-failed.png)
- **Models:** [the model that came with the update, marked new](evidence/phase-19/ai-tools-new-model.png) ·
  [and offered in the model menu](evidence/phase-19/ai-tools-new-model-menu.png)
- **The switch:** [Update AI tools by themselves, in Settings → Switches](evidence/phase-19/ai-tools-switch-in-settings.png)

Test totals: TOTALS.

On screen the plan's words become plain ones ([word list](../design/vocabulary.md)): "login" and
"logout" are **Sign in**, **Reconnect**, and **Sign out**, in a **sign-in tab**; "token usage" is
**tokens read, reused, and written**, with "tokens (pieces of words)" explained once; "quota" and
"rate-limit utilization" are **left of your plan**, **reported by … at …**; "billing mode" is
**How it is paid for: Subscription** and **Paid AI key (pay per use)**; "CLI version" and "tested
version" are **installed** and **checked by Plenipo**; "upgrade", "self-update", and "rollback"
are **a new version**, **Update**, **Updating…**, **Updated to …**, and **put back**;
"auto-update" is **Update AI tools by themselves**; and "unverified model" and "deprecated" are
**new — not checked yet** and **not offered by this version**. Quotes from the plan keep the
plan's words.

CI has no AI tool accounts. The end-to-end tests use Plenipo's stand-in AI tools
(`plenipo-fake-agent`), which now have sign-in and sign-out programs that wait for your key and
record every byte they receive, update commands (a new version, a failure, a version that does
not answer, a slow one, one that cannot update itself), Grok's and Claude Code's put-back
commands, Codex's app server, Claude Code's plan report, and model lists; a stand-in for the
release lists on npm and GitHub; and a throwaway home folder.

## 1. Acceptance criteria → evidence

The plan's one criterion, in its four parts:

| #   | Criterion (ROLLOUT_PLAN.md)                                                                    | Result   | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| --- | ---------------------------------------------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | The owner signs Codex out and back in without leaving Plenipo.                                 | **Pass** | E2E "signs Codex out and back in from its card, in a tab that runs only Codex's own command" ([tab](evidence/phase-19/ai-tools-sign-in-tab.png), [after](evidence/phase-19/ai-tools-signed-in-again.png)): `codex logout`, then `codex login`, in tabs of the terminal panel; the owner's own key finishes it; the card checks again by itself. Broker `sign_in_opens_a_tab_running_exactly_that_tools_login_command_and_nothing_else`, `after_the_tab_closes_the_card_checks_again_and_shows_the_new_sign_in`. |
| 2   | The owner sees this week's usage for Claude Code by model.                                     | **Pass** | E2E "shows this week's usage for Claude Code by model, and how much of the plan is left" ([usage](evidence/phase-19/ai-tools-usage.png)); broker `usage_totals_match_the_saved_turns`, `a_task_that_runs_past_midnight_on_two_models_is_counted_once`; `aiTools.test.tsx` "adds up the usage by model for today, this week, and last week; Kimi counts tasks only".                                                                                                                                             |
| 3   | The owner updates Grok with one click (or has it updated overnight) while no task is using it. | **Pass** | E2E "updates Grok with one click while no task uses it, and marks the model that came with it" ([ready](evidence/phase-19/ai-tools-update-ready.png), [updated](evidence/phase-19/ai-tools-updated.png)) and "an update waits while a task uses Grok" ([waiting](evidence/phase-19/ai-tools-update-waiting.png)); broker `new_versions_come_from_each_tools_own_check_or_its_makers_list` (with the switch on, Grok updates by itself), `an_update_never_starts_while_a_task_is_using_the_tool_it_waits`.       |
| 4   | … and sees a model that arrived with the update, marked as new.                                | **Pass** | E2E (the same test: `grok-5` marked **new — not checked yet** on the Models tab, [screenshot](evidence/phase-19/ai-tools-new-model.png), and offered as "grok-5 — new, not checked yet" in Settings → AI models → Add a model, [screenshot](evidence/phase-19/ai-tools-new-model-menu.png)); broker `after_an_update_the_version_sign_in_and_models_are_checked_again`.                                                                                                                                         |

## 2. Deliverables → evidence

| Plan deliverable                        | Built as                                                                                                                                                                                                                                                                                                                                                                                                                                                     | Tests                                                                                                                                                                                                                                                                                                                                          |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Sign in / Reconnect / Sign out**      | ADR-058: a tab in the terminal panel runs the tool's own command from a fixed list (`claude auth login`, `codex login`, `grok login`, `kimi login`, `ollama signin`, and their sign-outs; Kimi has none), directly, with no shell, with its tasks' cleared environment. Sign out and Reconnect wait while a task uses the tool (the wait is kept even when you leave the page), with Cancel. When the program ends, Plenipo checks the tool again by itself. | Contract `every_ai_tool_signs_in_with_its_own_command_and_never_for_api_billing`; broker `sign_in_opens_…`, `after_the_tab_closes_…`, `sign_out_waits_while_a_task_is_using_the_tool`, `an_update_and_a_sign_in_tab_take_turns_on_one_tool`; `aiTools.test.tsx` (sign-in tests); IPC; E2E.                                                     |
| **Usage**                               | ADR-060 §1–§3: tokens read (reused) and written, and tasks, by model, for today, this week, last week, and the last 14 days, added up from the counts saved with each step; the usage limit and its reset time with **Try again now**; "left of your plan" for Claude Code (from its task stream) and Codex (from its app server), with the time it was reported; the others say they don't report it.                                                       | Ledger `token_steps_are_the_counts_saved_with_each_step`; broker `usage_totals_…`, `a_task_that_runs_past_midnight_…`, `the_payment_switch_…_and_plans_come_only_as_reported`; runtime `the_plan_comes_through_the_stream_plenipo_already_reads`, `its_app_server_is_asked_for_the_plan_and_the_models_with_no_task`; `aiTools.test.tsx`; E2E. |
| **How it is paid for**                  | ADR-060 §4: **Subscription**, with the plan's name; the switch **Paid AI key (pay per use)** is shown, off, and locked, with "Comes with spending caps in a later version." The command refuses `paidKey`.                                                                                                                                                                                                                                                   | Broker `the_payment_switch_cannot_be_turned_to_a_paid_key_…`; IPC `the_phase_19_commands_check_what_they_are_given`; `aiTools.test.tsx` "the paid-key switch is locked, and Plenipo never asks for a paid key"; E2E.                                                                                                                           |
| **Version**                             | ADR-059 §1: "Installed 1.0.41 · Checked by Plenipo 1.0.41", and a notice when they differ (newer: it should work; older: update it to get every model).                                                                                                                                                                                                                                                                                                      | `aiTools.test.tsx` "shows installed and checked versions, and a notice when they differ"; E2E.                                                                                                                                                                                                                                                 |
| **Update**                              | ADR-059 §2–§10: the newest version (Grok's own check; npm's lists for Claude Code and Codex; GitHub's for Ollama; Kimi's own upgrade), **Update to …**, and each state: waiting (with Cancel), updating, checking, updated, up to date, by hand (with **Open a terminal**), failed (the old version still works, or put back, or given no tasks). A new task says it waits while the tool updates.                                                           | Contract `every_update_is_the_tools_own_command_and_asks_nothing`, `grok_says_its_newest_version_itself`; broker update tests (7); Guard `the_ai_tools_newest_versions_come_only_from_their_own_release_lists`; `aiTools.test.tsx`; `WorkersView.test.tsx` "says a new task waits while its AI tool is being updated …"; E2E.                  |
| **Models**                              | ADR-060 §5: the models Plenipo checked, with their effort levels, and the ones the tool reports (Codex's app server, Grok's and Kimi's ACP answers, Ollama's list), marked **new — not checked yet** and offered in every model menu; a checked one the tool no longer lists is **not offered by this version**; Claude Code's come with Plenipo's updates.                                                                                                  | Contract `each_tools_own_check_lists_its_models_with_no_task`; router `new_models_are_the_reported_ones_plenipo_has_not_checked`; `aiTools.test.tsx`; `ModelSettings.test.tsx`; E2E.                                                                                                                                                           |
| **Usage limits move from Settings**     | Settings → AI models: "Usage limits, sign-in, and updates are on the AI tools page", with **Open the AI tools page**; "When an AI tool reaches its usage limit" stays (a rule for roles).                                                                                                                                                                                                                                                                    | `ModelSettings.test.tsx` "links to the AI tools page, where the usage limits moved (Phase 19)"; `settings.test.tsx`; E2E.                                                                                                                                                                                                                      |
| **Updates: once a day, only when free** | ADR-059 §2–§3, §8: 4 minutes after Plenipo starts, then each day; **Update AI tools by themselves** (on the page and in Settings → Switches), off by default: off, Plenipo tells you (the card and a Windows notice); on, it updates each tool by itself, only when no task is using it.                                                                                                                                                                     | Broker `new_versions_come_from_…`, `with_the_switch_on_the_owner_still_hears_what_plenipo_cannot_update`; Ledger `an_ai_tools_new_version_and_its_update_make_notices`; `SwitchSettings.test.tsx`; E2E.                                                                                                                                        |

## 3. Plan tests → evidence

| Test (plan)                                                                                                       | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Sign in opens a terminal tab running exactly that tool's login command, and nothing else can be started that way. | Broker `sign_in_opens_a_tab_running_exactly_that_tools_login_command_and_nothing_else` (exactly `login`, nothing typed before a key, the terminal's own command refuses the place); IPC `the_phase_19_commands_check_what_they_are_given` (another tool, another action, arguments, a program: refused); `aiTools.test.tsx`; E2E (`last-args.json` is exactly `["login"]`, and only the owner's key reached the program). |
| After the tab closes, the card re-checks and shows the new sign-in state.                                         | Broker `after_the_tab_closes_the_card_checks_again_and_shows_the_new_sign_in`; contract `a_tool_is_checked_again_on_its_own_after_a_sign_in`; `aiTools.test.tsx` "after the sign-in tab ends, the card says Checking… until the new check arrives"; E2E.                                                                                                                                                                  |
| An update never starts while a task is using that tool; it waits.                                                 | Broker `an_update_never_starts_while_a_task_is_using_the_tool_it_waits`, `an_update_and_a_sign_in_tab_take_turns_on_one_tool`; contract `a_task_waits_while_its_ai_tool_is_held_and_a_busy_tool_is_not_held`, `a_sign_in_tab_left_open_holds_tasks_a_while_and_an_update_until_it_is_done`; E2E ([waiting](evidence/phase-19/ai-tools-update-waiting.png)).                                                               |
| A failed update leaves the old version working and says so.                                                       | Broker `a_failed_update_leaves_the_old_version_working_and_says_so`, `a_new_version_that_does_not_answer_is_put_back_or_given_no_tasks`; E2E ([failed](evidence/phase-19/ai-tools-update-failed.png)).                                                                                                                                                                                                                    |
| After an update, the version, sign-in, and models are re-checked.                                                 | Broker `after_an_update_the_version_sign_in_and_models_are_checked_again`; E2E (installed 1.1.0, the notice that it is newer than the checked version, and `grok-5`).                                                                                                                                                                                                                                                     |
| A model the tool reports but Plenipo has not checked shows as "new — not checked yet" and can be chosen.          | Router `new_models_are_the_reported_ones_plenipo_has_not_checked`; runtime `a_reported_model_is_kept_only_when_it_can_be_chosen`; `ModelSettings.test.tsx` "offers a model the AI tool reported as new, not checked yet; choosing it works like a typed name"; E2E ([menu](evidence/phase-19/ai-tools-new-model-menu.png)).                                                                                               |
| Usage totals match the saved turns; the limit and reset time show on the card.                                    | Broker `usage_totals_match_the_saved_turns` (added by hand and compared), `a_task_that_runs_past_midnight_on_two_models_is_counted_once`; `aiTools.test.tsx` "shows the usage limit with Try again now, …"; E2E ([limit](evidence/phase-19/ai-tools-usage-limit.png)).                                                                                                                                                    |
| The payment switch cannot be turned to a paid key before Phase 16.                                                | Broker `the_payment_switch_cannot_be_turned_to_a_paid_key_and_plans_come_only_as_reported`; IPC (`paidKey` refused from the main window, every command refused elsewhere); `aiTools.test.tsx` (the switch is disabled and the command is never called); E2E (the switch is shown and locked).                                                                                                                             |
| End-to-end tests in the real app, with screenshots in `evidence/phase-19/`.                                       | `tests/e2e/specs/ai-tools.e2e.mjs` (7 tests, 13 screenshots above). `agents.e2e.mjs` and `routing.e2e.mjs` follow the new cards and the moved usage limit.                                                                                                                                                                                                                                                                |

## 4. The owner's rules → evidence

| Rule                                                                                                                                                                                                                                    | Evidence                                                                                                                                                                                                                                                                                                                                                                                          |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Never ask for passwords, keys, tokens, or secrets in chat; never commit them.                                                                                                                                                           | This branch's history and PR #94.                                                                                                                                                                                                                                                                                                                                                                 |
| Files, programs, the network, the browser, and the screen go through Guard and the capability broker.                                                                                                                                   | Sign-in tabs are opened by the broker after `Guard::check_ai_tool_action` (refusals recorded as `guard.ai_tool_refused`), with the supervisor's approval of the program; updates and put-backs run through `programs::run` (shown in Runs); the release lists go through Guard's gate with the purpose **AI tool versions**, which allows exactly three addresses; no file of an AI tool is read. |
| New desktop commands are the main window's alone; the sign window and web pages are refused.                                                                                                                                            | IPC `the_ai_tools_page_is_the_main_windows_alone` (all 8 commands, and `open_terminal` with an AI tool's place: refused from another window, the sign window, and a web page), `the_phase_19_commands_check_what_they_are_given`. The commands run off the window's thread (`every_ai_tools_command_runs_off_the_windows_thread`).                                                                |
| Signing in only in a terminal tab that runs the tool's own login command, from a fixed list; the owner types; Plenipo never reads, stores, or passes the sign-in, never types into the tab, never reads the tools' saved sign-in files. | ADR-058; contract `every_ai_tool_signs_in_…`; broker `sign_in_opens_…` (nothing reaches the program before a key); E2E (only the owner's key reached it). What the tab shows goes only to the screen. The terminal's own automatic answers about the screen are the only other bytes, as in every terminal (ADR-058, as built).                                                                   |
| Plenipo never calls an AI tool's unpublished web addresses; "plan left" only when the tool reports it officially.                                                                                                                       | Guard `the_ai_tools_newest_versions_come_only_from_their_own_release_lists`; runtime `reads_the_plan_only_from_its_documented_rate_limit_event`, `reads_the_plan_and_the_models_it_reports_never_the_email`; `aiTools.test.tsx` (Grok, Kimi, and Ollama say they don't report it).                                                                                                                |
| Updates only with each tool's own official update command, never while a task uses it, asking first unless the switch is on; a failed update leaves the old version working.                                                            | ADR-059; contract `every_update_is_the_tools_own_command_and_asks_nothing`; broker update tests; the switch is off by default (`new_versions_come_from_…`).                                                                                                                                                                                                                                       |
| The paid-key switch cannot be turned on before Phase 16.                                                                                                                                                                                | Section 3.                                                                                                                                                                                                                                                                                                                                                                                        |
| Logs and diagnostics files never hold secrets or anything typed in the terminal.                                                                                                                                                        | A sign-in tab's output goes only to the screen; an update's output stays in memory (Runs); what is kept of a failure is its last line, filtered for secrets (the filter now knows xAI and npm tokens, `recognizable_secrets_are_hidden`); the diagnostics file lists versions and states only.                                                                                                    |
| No model names in commits, branch names, or pull requests.                                                                                                                                                                              | This branch's history and PR #94.                                                                                                                                                                                                                                                                                                                                                                 |
| Plain words on screen; ADRs named.                                                                                                                                                                                                      | [Word list](../design/vocabulary.md) gains the Phase 19 pairs; `aiTools.test.tsx` "names every control, with one main heading and no skipped level".                                                                                                                                                                                                                                              |

## 5. Deviations from the plan and the design

Each is recorded in its ADR's "As built" section.

- **Grok's models** come from its ACP `initialize` answer, not also `grok models` (ADR-060).
- **Models are first asked for 4 minutes after Plenipo starts**, with the first look for new
  versions, not the moment it starts (ADR-060).
- **Sign in waits too** while a task uses the tool (for Reconnect); a signed-out tool has no
  tasks, so plain Sign in never waits (ADR-058).
- **A sign-in tab holds new tasks for at most ten minutes**; an update holds them until it and its
  checks are done (ADR-058, ADR-059).
- **A task between steps** does not count as using the tool, so Sign out does not wait for it
  (ADR-058).
- **Numbers:** ADR-058 to ADR-060; no new Ledger layout (it stays at 11); 8 new desktop commands,
  and a new kind of place for `open_terminal`.

## 6. Defects found and fixed during Phase 19

A review across five areas (the runtime and the AI tools' commands; the AI tools service, Guard,
and the Ledger; the desktop commands; the page; and security), each finding checked by a second
reviewer before it was fixed. Every confirmed finding is fixed with a test, or recorded where it
is a limit of the design.

**Updates, sign-in, and checks**

- An update and a sign-in tab could run on one AI tool at the same time, and Plenipo's own short
  checks could run a tool while it updated (on Windows, a program in use cannot be replaced).
  Now they take turns: an update waits while the sign-in tab is open or a check runs, a sign-in
  tab waits while the tool updates (Guard says why), and checks skip a tool that is updating
  (`an_update_and_a_sign_in_tab_take_turns_on_one_tool`).
- A task that would start during an update waited only ten minutes, then could start on a tool
  still being updated. It now waits until the update and its checks are done
  (`a_sign_in_tab_left_open_holds_tasks_a_while_and_an_update_until_it_is_done`).
- A task waiting for its AI tool could not be stopped for up to 20 minutes. **Stop** now works
  while it waits (`the_owner_can_stop_a_task_that_waits_for_its_ai_tool`), and a continuing task
  no longer counts as using the tool before it starts
  (`a_continuing_task_waiting_for_its_ai_tool_is_not_counted_as_using_it`).
- A signed-out tool, or one slow to list its models, was taken for a broken update: Plenipo
  could put the old version back, or stop giving the tool tasks, for no reason. A tool that
  answers its check's first request now counts as answering, and answers read before the time
  limit are kept
  (`a_tool_that_answers_its_check_counts_as_answering_even_signed_out_or_slow`).
- A Cancel at the moment an update started could say "cancelled" while the update ran, and a new
  **Update** pressed right after a Cancel was lost. Both now settle under one lock.
- **Update** on a tool that is not installed could mark it "not given tasks" until the owner
  pressed Check again. It is now refused, and an update that never started changes nothing.
- With the switch on, a new Ollama version was never announced (Plenipo cannot update Ollama),
  and an update a tool cannot do by itself was retried every day without a word. Ollama's are
  announced, and the other is said once (`with_the_switch_on_the_owner_still_hears_what_plenipo_cannot_update`).

**What Plenipo keeps and counts**

- If what Plenipo keeps for the page could not be read once, it was written over with nothing,
  turning off the owner's switch. It is now read and written in one step, and never written over
  when it cannot be read (`what_plenipo_keeps_but_cannot_read_is_never_written_over`,
  `a_setting_plenipo_keeps_changes_in_one_step_and_a_refused_change_writes_nothing`).
- A task that ran past midnight, or on two models, was counted twice. Each task now counts once
  (`a_task_that_runs_past_midnight_on_two_models_is_counted_once`); with very many steps, the
  newest are kept, not the oldest.
- A model a tool reported could be offered but not chosen (a name Plenipo refuses). Only names
  that can be chosen are kept (`a_reported_model_is_kept_only_when_it_can_be_chosen`).

**The desktop and security**

- Three of the page's commands read the Ledger on the window's own thread; a busy Ledger could
  freeze the window. All eight now run off it
  (`every_ai_tools_command_runs_off_the_windows_thread`), and a plan report is saved off the
  task's thread.
- A desktop test could fail by chance (it started an update, then expected none to be waiting).
- A version passed to a put-back command is now exactly numbers and dots; a failed check's text
  is filtered for secrets before it is kept; the filter now knows xAI's keys and npm's tokens.
- A long line from a tool's check could fill memory; one line is now at most 1 MB.

**The page**

- A Sign out waiting for a task was forgotten when the card showed another tab, or the owner
  left the page. The wait is now kept with the terminal panel, and still opens the tab.
- Retrying a refused tab had no end; it stops after three refusals with nothing changing and
  offers **Try again**. A tab it opens by itself does not take the keyboard.
- A tab that opened after **Try again** could be opened a second time.
- A broken tool read "not installed"; it now says it is not working, and why.
- "Limit reached" showed on every window of Codex's report; it now shows once, and each window
  keeps its share left.
- A pre-release could be offered as an update, though Plenipo never counts it newer.
- "Today" did not move on at midnight with the page open.
- Codex's card said its plan comes "during a task"; it comes from Plenipo's check.
- The five paid-key switches had the same name for screen readers; the keyboard was lost when
  Sign out started waiting.
- Found in the screenshots: a sign-in tab ended with "The shell ended" (it runs no shell; it now
  says "Codex's sign-in ended"), and a plan line read "Your plan: 91% of your plan left".

All of these have tests in `aiTools.test.tsx`, `panel.test.ts`, `WorkersView.test.tsx`, and the
Rust tests named above. Each test fails without its fix.

**Recorded, not changed** (limits of the design, in the ADRs' as-built notes)

- A task between steps does not count as using the tool, so Sign out does not wait for it.
- While one AI tool updates, tasks waiting for it count toward the tasks Plenipo runs at once.
- The terminal's own automatic answers about the screen reach the sign-in program, as in every
  terminal.
- Two findings were not defects: the Codex check's `account/read` is part of the accepted design
  (only the kind of sign-in and the plan's name are kept), and the checks run as approved
  programs without a Runs entry, like the checks before every task (the ADR's words are fixed).

## 7. Left for the owner (on Windows)

- **The walk-through** on a real PC with real AI tools:
  - Sign Codex out and back in from its card.
  - Look at this week's usage for Claude Code, by model.
  - Update Grok with one click while no task uses it (or turn on **Update AI tools by
    themselves** and let it update by itself), and see a model that came with the update marked
    as new.
- **Checks on the real programs:**
  - `codex update` on your Codex install (npm): does it update, or print npm's command?
  - `grok update` with `GROK_DISABLE_AUTOUPDATER=1` set: does it still update when asked?
  - Whether Kimi updates itself during a task (and `kimi upgrade --help`).
  - Claude Code's `rate_limit_event` on your plan: does it carry `utilization`?
  - Codex's app server on your Codex version: do `account/rateLimits/read` and `model/list`
    answer?
- **The upgrade from 1.11.0**: a backup is taken first; the Ledger layout does not change.
