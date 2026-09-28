# Phase 17 — Acceptance Report

|              |                                                                                                                                                                                                                                                                                                                                                  |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Phase**    | 17 — The Owner's Control Over Workers                                                                                                                                                                                                                                                                                                            |
| **Branch**   | `claude/phase-17` ([PR #77](https://github.com/Seckcey/plenipo/pull/77))                                                                                                                                                                                                                                                                         |
| **Verified** | {VERIFIED}                                                                                                                                                                                                                                                                                                                                       |
| **Date**     | 2026-09-27 (Pacific time)                                                                                                                                                                                                                                                                                                                        |
| **Result**   | All six acceptance criteria and every Phase 17 test in the plan pass; every deliverable is built. Version **1.10.0**. Decisions: ADR-041 to ADR-044, accepted by the owner, and ADR-045, the owner's addition; all built, with the differences each records as built. The walk-through with real AI tools on Windows is the owner's (section 8). |

Screenshots (from the end-to-end run in the real app, `tests/e2e/specs/control.e2e.mjs`):

- **Settings → AI models:** [Model and effort rules](evidence/phase-17/control-rules-settings.png)
  (the organization at high effort, and every role's reason naming the rule)
- **An agent's details, tab by tab:** [Overview](evidence/phase-17/control-panel-overview.png) ·
  [Job](evidence/phase-17/control-panel-job.png) (the Database specialty, learning off for this
  agent) · [AI model](evidence/phase-17/control-panel-ai-model.png) (max effort, "from this
  agent's own setting") · [Work](evidence/phase-17/control-panel-work.png) (its permissions) ·
  [Team](evidence/phase-17/control-panel-team.png) ·
  [Manage, widened](evidence/phase-17/control-panel-manage-wide.png)
- **Archive and delete for good:** [the Archived list](evidence/phase-17/control-archived-list.png) ·
  [Delete for good, offering the Workforce](evidence/phase-17/control-delete-for-good.png) ·
  [a whole department archived](evidence/phase-17/control-archived-department.png)
- **The Workforce:** [an agent saved, ready to hire again](evidence/phase-17/control-workforce.png)

Test totals: {TOTALS}

On screen the plan's words become plain ones ([word list](../design/vocabulary.md)): a "layer"
is a **rule** ("the Development department's rule"), a "position" is an **agent** on the chart,
"restore" is **Bring back**, "hard delete" is **Delete for good**, a "tombstone" is a **short
record**, a "brief" is the **full instructions** or a **short reminder**, "compaction" is
**shortened its memory**, and the owner's "score" is **Experience**. Quotes from the plan keep
the plan's words.

CI has no AI tool accounts. The end-to-end tests use Plenipo's stand-in AI tool
(`plenipo-fake-agent`), which reports the effort it was given, and a throwaway home folder.

## 1. Acceptance criteria → evidence

| #   | Criterion (ROLLOUT_PLAN.md)                                                                                                                                                   | Result   | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | The owner sets a model and effort for the whole organization, overrides it for one department and for one agent, and the Router's reason names each layer.                    | **Pass** | Router `each_layer_sets_model_and_effort_and_the_closest_wins` (organization, department, role, agent; the reason names the rule that decided); E2E "rules in layers" (the organization at high effort, then one agent at max: "from this agent's own setting", [screenshot](evidence/phase-17/control-panel-ai-model.png)); IPC `archive_bring_back_and_delete_for_good_through_ipc` (a department's rule); Workforce `a_moved_agent_takes_its_new_departments_effort`. |
| 2   | The owner turns learning off for one agent while the rest keep learning.                                                                                                      | **Pass** | Workforce `learning_follows_the_closest_setting_under_the_main_switch` (off for one agent, on for one agent inside a role that is off, and the main switch over all); E2E "learning off for one agent" ([Job tab](evidence/phase-17/control-panel-job.png)).                                                                                                                                                                                                             |
| 3   | The owner hires a Senior Developer with the Database specialty.                                                                                                               | **Pass** | Workforce `a_senior_developer_with_the_database_specialty` (its lines reach the worker's instructions, the team list names it); E2E "hires a Senior Developer with the Database specialty" (the Hire box suggests "Database Developer").                                                                                                                                                                                                                                 |
| 4   | The owner archives an agent, brings it back, archives it again, and deletes it for good; older activity still shows its name.                                                 | **Pass** | E2E "archive, bring back, and delete for good" ([Archived list](evidence/phase-17/control-archived-list.png), [Delete for good](evidence/phase-17/control-delete-for-good.png)); Ledger `deleting_for_good_leaves_a_short_record_that_still_names_it`; Workforce `archive_bring_back_delete_and_the_workforce` (older tasks still name it).                                                                                                                              |
| 5   | The average prompt size on routine turns falls, measured before and after.                                                                                                    | **Pass** | Section 5: a new routine objective carries 73% less of Plenipo's own text than before (6,432 → 1,756 bytes), and a step that delivers replies 45% less (2,013 → 1,106). Liaison `a_supervisors_conversation_is_measured_step_by_step`.                                                                                                                                                                                                                                   |
| 6   | (ADR-045) Deleting a department for good offers to save its experienced agents; one saved to the Workforce is hired again into another team, with its experience and lessons. | **Pass** | IPC `archive_bring_back_and_delete_for_good_through_ipc` (a department deleted for good, its developer saved and hired back into a new department); Workforce `archive_bring_back_delete_and_the_workforce` (experienced agents checked, experience and lessons back); E2E "archive, bring back, and delete for good" ([Workforce](evidence/phase-17/control-workforce.png)); `OwnerControl.test.tsx`.                                                                   |

## 2. Deliverables → evidence

| Plan deliverable                         | Built as                                                                                                                                                                                                                                                                                                                                                      | Tests                                                                                                                                                                                                                                |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **effort per agent**                     | The AI model tab's **Effort** (its own rule's effort), with or without a fixed AI tool and model. Changing only the effort keeps the agent and its conversation: the open conversation takes the new effort from its next task (`refresh_open_efforts`, one refresh at a time).                                                                               | Workforce `changing_only_effort_keeps_the_agent_and_its_conversation`, `an_open_conversation_takes_the_effort_set_for_its_listed_model`; `Inspector.test.tsx` (effort for the model it runs); E2E "rules in layers".                 |
| **model and effort rules in layers**     | ADR-041: Settings → AI models → **Model and effort rules** (the organization, each department, each role, each agent with its own); the closest wins; AI companies never to use add up (the owner's choice). A fixed AI tool a rule never uses is refused when saved, and shown as unable to start.                                                           | Router `each_layer_sets_model_and_effort_and_the_closest_wins`, `never_used_companies_add_up_across_layers`; Workforce `a_fixed_ai_tool_a_rule_never_uses_is_refused_in_plain_words`; `ModelSettings.test.tsx` rules; E2E.           |
| **learning in layers**                   | Worker learning (Settings → Switches) stays the main switch; each role and each agent learns or not; the agent's own setting wins, then its role's. Each role's "keep lessons without asking" stays.                                                                                                                                                          | Workforce `learning_follows_the_closest_setting_under_the_main_switch`; IPC `learning_through_ipc`; E2E.                                                                                                                             |
| **specialties under each role**          | ADR-042: 20 built-in specialties (Senior Developer 7, Designer 3, Security Auditor 2, Operations Engineer 4, Researcher 2, Documentation Writer 2) and the owner's own on any role; each adds lines and suggests models and permissions (suggestions never change anything on their own).                                                                     | Ledger `built_in_specialties_are_seeded_once_and_kept_up_to_date`, `the_owner_adds_changes_and_removes_specialties_on_any_role`; Workforce `a_senior_developer_with_the_database_specialty`; E2E.                                    |
| **archive, bring back, delete for good** | ADR-043: agents, projects, and departments; the List view's **Archived** tab; what was archived together comes back together; delete for good asks first, is refused while anything has unfinished work, keeps a short record in the same Ledger row, and never touches files. Archive and delete go no further than what they name.                          | Ledger owner-control tests (15); Workforce `archive_bring_back_delete_and_the_workforce`; IPC; `OwnerControl.test.tsx`; E2E (an agent, and a whole department).                                                                      |
| **the properties panel rebuilt**         | Six tabs (Overview, Job, AI model, Work, Team, Manage); a line under every option saying what it does; effort and permissions shown; the panel widens from its edge (or with the arrow keys) and remembers its width.                                                                                                                                         | `Inspector.test.tsx` (every option's line, against the word list; widening); E2E (every tab, widened).                                                                                                                               |
| **prompts sized to the job**             | ADR-044: every step's size is measured and recorded; routine tasks get a short reminder; the full instructions go out at the start, after a shortened memory, every 10th objective, for a large job, and when they changed; handoffs are short, labeled, in plain words, and point at saved records by ID where the AI tool says when it shortens its memory. | Runtime `routine_objectives_get_a_short_reminder_and_the_rest_the_full_instructions`, `a_shortened_memory_is_heard_…`, `after_a_restart_…`, `the_permissions_note_goes_out_in_full_only_when_needed`; Liaison prompt-size tests (5). |
| **experience and the Workforce**         | ADR-045: **Experience** is 10 for each kept lesson and 1 for each finished task; deleting for good checks the agents above the average; the List view's **Workforce** tab; hiring from it brings back its settings, experience, and lessons.                                                                                                                  | Ledger `experience_counts_kept_lessons_and_finished_tasks`, `a_saved_agent_leaves_a_short_record_…`; snapshot `experienced_means_above_the_exact_average_…`; Workforce; IPC; `OwnerControl.test.tsx`; E2E.                           |

## 3. Plan tests → evidence

| Test (plan)                                                                                                                      | Evidence                                                                                                                                                                                                            |
| -------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Each layer sets model and effort, and the closest wins; the routing reason names the layer.                                      | Router `each_layer_sets_model_and_effort_and_the_closest_wins`, `effort_comes_from_the_role_then_the_model`; E2E "rules in layers".                                                                                 |
| An effort not accepted by the model is refused with a plain message.                                                             | Router `efforts_are_refused_in_plain_words_naming_what_the_model_takes`, `role_efforts_are_validated_and_follow_their_model`; Workforce (ultra refused: "does not take ultra effort").                              |
| Changing only effort does not hire a new agent.                                                                                  | Workforce `changing_only_effort_keeps_the_agent_and_its_conversation` (same agent, same conversation, `--effort high` on the next task); `Inspector.test.tsx`; E2E.                                                 |
| Learning off at the organization stops all learning; off for one agent only that agent; the closest wins.                        | Workforce `learning_follows_the_closest_setting_under_the_main_switch`.                                                                                                                                             |
| A specialty's lines reach the worker's instructions; a position without one gets the role alone.                                 | Workforce `a_specialtys_lines_reach_the_workers_instructions`, `a_senior_developer_with_the_database_specialty`.                                                                                                    |
| Archive → bring back restores the agent; delete for good leaves a short record that old activity names.                          | Ledger `an_agent_comes_back_as_it_was_with_its_assignments`, `a_department_is_archived_with_everything_in_it_and_comes_back_whole`, `deleting_for_good_leaves_a_short_record_that_still_names_it`; E2E.             |
| Delete for good is refused while there is unfinished work.                                                                       | Ledger `deleting_for_good_is_refused_while_anything_has_unfinished_work`; Workforce.                                                                                                                                |
| Routine turns carry the short reminder; the first turn, a shortened memory, and a large job the full brief.                      | Runtime `routine_objectives_get_a_short_reminder_and_the_rest_the_full_instructions`, `a_shortened_memory_is_heard_and_the_next_task_gets_the_full_instructions`; Liaison `a_large_job_gets_the_full_instructions`. |
| Each turn's prompt size is recorded.                                                                                             | Liaison `a_supervisors_conversation_is_measured_step_by_step` (what the AI tool received equals what was recorded, on the task and its execution); `format.test.ts` (shown beside the tokens).                      |
| The properties panel's every option has its one-line explanation (snapshot against the word list).                               | `Inspector.test.tsx` "has six tabs, and every option on every tab says what it does" (snapshot; no word from the list's "Not" column).                                                                              |
| Experience counts kept lessons and finished tasks; deleting for good offers, saves, and deletes; hiring back brings it all back. | Ledger `experience_counts_kept_lessons_and_finished_tasks`, `a_saved_agent_leaves_a_short_record_and_is_hired_again_with_its_experience`; Workforce `archive_bring_back_delete_and_the_workforce`; IPC; E2E.        |
| End-to-end tests in the real app, with screenshots.                                                                              | `tests/e2e/specs/control.e2e.mjs` (5 tests, 11 screenshots above), and the whole end-to-end suite against the release build.                                                                                        |

## 4. The owner's decisions and rules → evidence

| Decision or rule                                                                                     | Evidence                                                                                                                                                                                                                                        |
| ---------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AI companies never to use **add up** across the rules (ADR-041 §4).                                  | Router `never_used_companies_add_up_across_layers`; Workforce `a_fixed_ai_tool_a_rule_never_uses_is_refused_in_plain_words`.                                                                                                                    |
| Deleting a project or department takes along what was archived with it (ADR-043 §12).                | Ledger `a_department_is_archived_with_everything_in_it_and_comes_back_whole`; IPC; and it goes no further: `archiving_or_deleting_goes_no_further_than_what_it_names`, `a_position_deleted_for_good_takes_no_other_departments_lead`.           |
| ADR-045 as written, experienced agents **checked** by default.                                       | `OwnerControl.test.tsx` "asks before deleting for good, and saves the experienced agents unless told otherwise"; E2E.                                                                                                                           |
| New desktop commands are the main window's alone; the sign window and web pages are refused.         | IPC `the_owners_control_over_workers_is_the_main_windows_alone` (all 15 commands, refused from another window, the sign window, and a web page) and `the_phase_17_commands_check_what_they_are_given` (each bad input refused, for its reason). |
| Delete for good is refused while anything has unfinished work, keeps a short record, and asks first. | Ledger tests; `OwnerControl.test.tsx` (nothing is deleted until you confirm); E2E. A deleted department's permission limit goes with it (IPC, Guard `a_department_deleted_for_good_holds_no_permission_set`).                                   |
| Files, programs, the network, the browser, and the screen go through Guard and the broker.           | This phase adds no such path: deleting for good never touches the disk, and prompt sizes are numbers.                                                                                                                                           |
| Logs and diagnostics files never hold secrets or anything typed in the terminal.                     | Prompt sizes are numbers only; no prompt text reaches the Ledger, events, or logs (Liaison size test checks what is recorded).                                                                                                                  |
| No secrets asked for in chat; none committed. No model names in commits or pull requests.            | This branch's history and PR #77.                                                                                                                                                                                                               |
| Plain words on screen; ADRs named.                                                                   | [Word list](../design/vocabulary.md) gains the new pairs; `Inspector.test.tsx` checks every option against it.                                                                                                                                  |

## 5. Prompts sized to the job: measured before and after

Measured by the same test, `a_supervisors_conversation_is_measured_step_by_step` (Liaison): a
full-time Supervisor's own conversation, with instructions and a permissions note as long as the
Workforce and the capability broker write them, running on the stand-in AI tool. "Before" is the
same test on the code that measured first and still sent everything every time (commit `bb7ebb0`,
ADR-044 §1.3); "after" is this release. Plenipo's own text is everything except what it only
passes along (the objective, context from another worker, and replies).

| Step                                                  | Before (bytes of Plenipo's own text) | After                | Less                         |
| ----------------------------------------------------- | ------------------------------------ | -------------------- | ---------------------------- |
| The first objective (the full instructions and note)  | 6,432 (6,494 in all)                 | 6,666 (6,728 in all) | — (more: the labeled format) |
| A new routine objective in the same conversation      | 6,432                                | 1,756                | **73%**                      |
| The step that delivers a reply, in the same objective | 2,013                                | 1,106                | **45%**                      |

Within the same run, each step also records what its own text would have been with the full
instructions and note: a new routine objective 6,666 → 1,756 bytes (74% less), a reply step
2,128 → 1,106 (48% less). The plan's goal, at least half off on routine tasks, is met.

Each new objective has its own working copy and branch, so its permissions note is new and goes
out in full; the instructions go out as a short reminder. A reply step in the same objective
carries the short note too. (The first measurement in the review gave the same note to every
objective and reported 89%; the review corrected the test, section 7.)

## 6. Deviations from the plan

- **Authorized penetration testing is not a built-in Security Auditor specialty.** The plan lists
  it; Plenipo ships 20 built-in specialties, not 21. You can add it yourself as one of your own
  specialties, with your own lines, on the Security Auditor role (or any role); Guard still
  decides what that worker may do, and asks you before each program and each server as your
  permission sets say. Recorded in ADR-042 (specialties), "As built".
- **Settings shows each rule's choices, not each rule's next worker.** What an agent's next
  worker gets, and which rule decided, shows on each role's line in Settings → AI models and in
  each agent's details. Recorded in ADR-041 (model, effort, and learning in layers), "As built".
- **Prompts sized to the job, as built** (recorded in ADR-044, "As built"): when permissions
  change, the full permissions note goes out and the rest of the instructions stay a short
  reminder; Kimi's and Grok's saved records are pasted every time, as for Codex, because they
  report their memory only after each answer; once an Ollama conversation is longer than
  Plenipo's helper sends at once, every task goes out in full.
- **Numbers for the records and the Ledger's layout:** ADR-041 to ADR-045. The security fixes
  merged into `main` meanwhile as ADR-046 to ADR-051 and took Ledger layout 9 (each lesson's
  project, ADR-050); Phase 17's layout is 10. With ADR-050, an agent saved to your Workforce
  remembers which of its lessons you never reviewed, and they come back that way, for the
  project it joins.

## 7. Defects found and fixed during Phase 17

A review across five areas (the Ledger, the rules, security, the screens, and prompts sized to
the job), each finding checked by a second reviewer before it was fixed. Every confirmed finding
is fixed with a test, or recorded where it is a limit of the design.

**The Ledger**

- Archiving a department could take along a manager of another department who reported to
  someone in it, and deleting for good could delete another department's lead. Both are refused
  now, saying what to do; a department or project comes back only with its lead.
- "Experienced" compared with the rounded average (an 11 was not above 10.5); it uses the exact
  one.
- The Ledger's guard on archived agents now also keeps their title, role, and dates.
- An assignment met from both sides when archiving ended once but was recorded twice.
- Hiring from the Workforce now records the settings and experience it came back with.
- Older calls that deleted departments and projects outright are for tests only.

**The rules**

- An open conversation's effort was looked up by the model name the AI tool reported, which can
  differ from the name in your list, so an effort set for that model could be missed. Moving an
  agent to another department did not update its effort.
- A fixed AI tool that a rule never uses could be saved and then never start; it is refused, and
  one fixed before the rule shows it cannot start, and why.
- A rule left empty when its only model left your list stayed as an empty "own rule"; one
  conversation that could not take a new effort stopped the others; a specialty that suggested a
  model since removed could no longer be saved.

**Security**

- Delete for good reported a failure if forgetting the department's permission limit failed,
  though the delete was done; it now finishes, and Guard drops such a leftover limit. A deleted
  department can no longer be given a limit.
- The input tests now check each refusal's reason; a specialty's suggestions are counted before
  they are read; the architecture overview lists the 15 new commands.

**The screens**

- The Effort menu could show one effort while the agent ran at another (an effort set for its
  model won); Effort and "Use these models" could be undone by saving the rule editor open below
  them; the menu briefly jumped back after saving; a missing model settings file hid the section
  without a word.
- "Chosen by" always named the role; an archived automatic agent read as broken; a supervisor
  whose project went with its department was offered a Bring back that was refused; an on-call
  agent showed an empty People section; the move form kept its old choice; "Hire a VP" started
  with a Worker role; project counts included deleted projects; the Delete for good box listed
  the item as going with itself and read as if it had already happened.

**Prompts sized to the job**

- A task that failed after sending new instructions left the older ones counted as delivered;
  the next task now sends them in full.
- An Ollama conversation too long for Plenipo's helper lost its start, and its instructions,
  for one task before Plenipo noticed; Plenipo now checks before sending.
- The measurement gave every objective the same permissions note (section 5).
- The Workers page showed one step's tokens beside all steps' size; it adds up both the same way.

**After merging the security fixes from `main`**

- The Workforce copied an agent's kept lessons and brought them back as kept by you, for every
  project, even ones its role had kept on its own that you never reviewed. They now come back as
  they were kept, for the project the agent joins (ADR-045 and ADR-050, as built).
- The fence around kept lessons got a new code every time the instructions were built, so every
  task of a role with kept lessons would have carried the full instructions. The code now stays
  the same while the lessons do, within a run of Plenipo, and is still unknown to anyone who
  writes a lesson (ADR-050, as built).
- On GitHub, the browser and servers end-to-end tests typed an objective while the details
  panel showed another tab (it keeps the tab last shown); every test now opens the Overview tab
  first.

During the build, the end-to-end run found the panel's tabs could not be clicked when they did
not fit (they wrap now), and a hint placed inside a label broke older tests (labels keep their
words; hints follow them).

## 8. Left for the owner (on Windows)

- **The acceptance walk-through on your PC with your real AI tools:** a rule for the
  organization, one department, and one agent; learning off for one agent; a Database developer;
  archive, bring back, archive again, and delete for good (saving one agent to your Workforce
  and hiring it again); and archiving a whole department.
- **Whether each AI tool's "shortened its memory" signal arrives:** Claude Code's notice when it
  compacts a long conversation (the next task's size then says "full instructions"), and Kimi's
  and Grok's context reports.
- **Upgrading from 1.9.0:** the first start backs up your Ledger ("Before a new version") and
  updates it to layout 10; your organization and work carry over.
