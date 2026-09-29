# Phase 16 — Acceptance Report (Wave 1)

|              |                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 16 — Every AI Model Worth Having, **Wave 1** ("fits today's rules, no paid key")                                                                                                                                                                                                                                                                                                                                                                                                               |
| **Branch**   | `claude/vigilant-ramanujan-c3o0bs` ([PR #97](https://github.com/Seckcey/plenipo/pull/97))                                                                                                                                                                                                                                                                                                                                                                                                      |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace`, `pnpm bindings` (no diff), and the end-to-end tests for model routing, AI workers, and the AI tools page against the release build (section 3). GitHub CI on PR #97: every check green on the merged commit (Rust, Frontend, Docs, Website, E2E on Linux, and Windows); the end-to-end job passed on its one re-run after a canvas filter test this wave doesn't touch timed out once.      |
| **Date**     | 2026-09-29 (Pacific time)                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| **Result**   | Every Wave 1 deliverable is built except more Ollama cloud models, which wait for the owner's paid plan (a follow-up). Every Wave 1 test in the plan passes. Version **1.14.0**. Decisions: ADR-080 and ADR-081 accepted by the owner (choices 1 to 3 as recommended, and the go-ahead to build); ADR-082 (Antigravity as an AI tool) proposed, built at the owner's direction, waiting for the owner's acceptance. The walk-through with real AI tools on Windows is the owner's (section 7). |

Screenshots (from the end-to-end run in the real app, with the stand-in AI tools):

- **Your models, grouped:** [by who made them](evidence/phase-16/models-grouped-by-maker.png) ·
  [by AI tool](evidence/phase-16/models-grouped-by-tool.png)
- **Model menus:** [Claude Code's, each name with its exact version and who made it](evidence/phase-16/models-add-menu.png) ·
  [Antigravity's, three companies](evidence/phase-16/models-add-menu-antigravity.png)
- **Antigravity:** [its card's Models tab](evidence/phase-16/ai-tools-antigravity-models.png) ·
  [a task on it, text only](evidence/phase-16/worker-result-antigravity.png) ·
  [every AI tool, six of them](evidence/phase-16/agent-runtimes.png)

## 1. What Wave 1 set out to do → result

| #   | Wave 1 (ROLLOUT_PLAN.md)                                                          | Result                  | Evidence                                                                                                                                                                                                                                                                            |
| --- | --------------------------------------------------------------------------------- | ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `maker` on every known model: who made it, separate from the AI tool that runs it | **Done**                | `KnownModel::maker`, `RuntimeCapabilities::{default_maker, runs_other_makers}`; contract `every_listed_model_says_who_made_it`; `crates/router/src/makers.rs`; the **Who made it** column and made-by words on every list.                                                          |
| 2   | Cross-company review counts the maker, not the AI tool                            | **Done**                | Router `the_same_maker_on_two_ai_tools_counts_as_one_company`, `two_makers_inside_ollama_count_as_two`, `a_model_whose_maker_is_not_known_plays_safe`; Liaison `the_directory_hears_the_model_that_did_the_work_under_review`, `the_directory_hears_the_model_of_a_task_passed_on`. |
| 3   | The model list groupable by maker or by the app that runs it, the owner's choice  | **Done**                | Settings → AI models → **Your models**, **Group by: Who made it / AI tool**, remembered on this PC. `ModelSettings.test.tsx`; E2E "Your models can be grouped by who made them or by AI tool, and the choice is kept".                                                              |
| 4   | Exact Claude model versions beside the plain names                                | **Done**                | `points_to` on `fable`, `opus`, `sonnet`, `haiku`, and the four exact versions listed (Part A on the owner's PC, [evidence](evidence/phase-16/README.md)); the menus say "opus — now Opus 5.5 — made by Anthropic".                                                                 |
| 5   | The older OpenAI models a ChatGPT sign-in really allows, each checked             | **Done — none allowed** | Part B on the owner's PC: all twelve older models refused on Codex 0.145.0 and 0.159.0. Codex's newest model (GPT-6.1-Sol) joins its list instead.                                                                                                                                  |
| 6   | Google's Gemini CLI as an AI tool, or a written finding                           | **Finding**             | [ai-tools-gemini-finding.md](ai-tools-gemini-finding.md): Google no longer serves it to personal accounts.                                                                                                                                                                          |
| 6a  | At the owner's direction: Google's Antigravity CLI in its place                   | **Done**                | ADR-082 (Antigravity as an AI tool): the adapter, its stand-in, the whole contract suite, and E2E. Step 0 on the owner's PC (Part E, E1 to E9).                                                                                                                                     |
| 7   | More Ollama cloud models once the owner's paid plan is active                     | **Follow-up**           | The paid plan starts 2026-09-30 (choice 4); Part D and those models come in a small follow-up.                                                                                                                                                                                      |

## 2. The owner's choices → as built

| Choice                                   | Answer                       | As built                                                                                                                                                                                                        |
| ---------------------------------------- | ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. Which list gets **Group by**          | As recommended: Your models  | Your models, grouped either way; every other list (each AI tool's Models tab, every model menu) says who made each model.                                                                                       |
| 2. "Never use" counts who made it too    | As recommended: yes          | A company on the list is skipped whether it made the model or its AI tool runs it; the list offers every company Plenipo knows. Also for a position fixed to a model.                                           |
| 3. A model whose maker is not known      | As recommended: play it safe | "Not known"; a must-be-different review never uses it; prefer-different tries it last; work by it never counts as another company's. **Also:** a role with companies never to use skips it (ADR-081, as built). |
| 4. Ollama's paid plan                    | Not yet (starts 2026-09-30)  | A follow-up.                                                                                                                                                                                                    |
| 5. Antigravity CLI in Gemini CLI's place | Yes, now                     | ADR-082.                                                                                                                                                                                                        |

## 3. Plan tests → evidence

| Test (plan and the owner's list)                                                       | Evidence                                                                                                                                                                                                                                                                                      |
| -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Two models with the same maker on different AI tools count as one company.             | Router `the_same_maker_on_two_ai_tools_counts_as_one_company` (Kimi K3 on Kimi and on Ollama).                                                                                                                                                                                                |
| Two makers inside Ollama count as two.                                                 | Router `two_makers_inside_ollama_count_as_two` (DeepSeek and Z.ai).                                                                                                                                                                                                                           |
| The model list groups correctly by maker and by AI tool, with the same models in both. | `ModelSettings.test.tsx` "groups your models by who made them or by AI tool, the same models both ways, and remembers the choice"; E2E (the same models both ways, the choice kept after a reload; [screenshot](evidence/phase-16/models-grouped-by-maker.png)).                              |
| The contract suite still refuses key variables for every subscription AI tool.         | Contract `no_credentials_are_passed_through` and the rest of the suite, now for six AI tools; Antigravity's own test shows no key variable reaches it.                                                                                                                                        |
| With the paid switch off, nothing changes.                                             | The paid-key switch is still locked (`aiTools.test.tsx` "the paid-key switch is locked…", now counting six tools; E2E); API billing stays off; a tool signed in with a key is still skipped (contract `sign_in_check_tells_a_subscription_from_an_api_key`, Antigravity's key list included). |
| Vitest for the page.                                                                   | `ModelSettings.test.tsx` (grouping, menus, never-use offering makers), `aiTools.test.tsx` (made-by and "now" on the Models tab, Antigravity's sign-in), `OrganizationView.test.tsx`, `WorkersView.test.tsx`.                                                                                  |
| End-to-end tests in the real app, with screenshots in `evidence/phase-16/`.            | `routing.e2e.mjs` (menus and grouping), `agents.e2e.mjs` (six AI tools, an Antigravity task), `ai-tools.e2e.mjs` (Antigravity's card and its settings file). The screenshots above.                                                                                                           |
| Antigravity passes the whole contract suite with its own persona.                      | Contract suite (17 tests), including `antigravity_runs_with_a_home_folder_of_its_own`.                                                                                                                                                                                                        |

## 4. The owner's rules → evidence

| Rule                                                                                                  | Evidence                                                                                                                                                                                                                                                                      |
| ----------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Never ask for passwords, keys, tokens, or secrets in chat; never commit them.                         | The checks on the owner's PC used a made-up key only; the owner's screenshots with an email address were not kept.                                                                                                                                                            |
| Files, programs, the network, the browser, and the screen go through Guard and the capability broker. | Wave 1 adds no new reach. Antigravity runs only as an approved program through the supervisor; its sign-in and update tabs go through `Guard::check_ai_tool_action`; it gets no Plenipo tools. The one file Plenipo writes for it is its settings, in Plenipo's own app data. |
| Nothing loads code into Plenipo while it runs.                                                        | Every maker, model, and version is compiled in.                                                                                                                                                                                                                               |
| New desktop commands are the main window's alone.                                                     | No new desktop command: the grouping is a stored choice on the page, like the theme.                                                                                                                                                                                          |
| Logs and diagnostics never hold secrets or anything typed in the terminal.                            | Nothing new is logged; Antigravity's sign-in tab output goes only to the screen, as for every AI tool.                                                                                                                                                                        |
| No model names in commits, branch names, or pull requests.                                            | This branch's history and PR #97.                                                                                                                                                                                                                                             |
| Plain words on screen; ADRs named.                                                                    | [Word list](../design/vocabulary.md) gains the new pairs; the reasons never say "maker" (router test); "Antigravity CLI" only in install steps.                                                                                                                               |
| Nothing in Connections, the Vault, or Phase 20's tests.                                               | No such file changed (`git diff origin/main --name-only`).                                                                                                                                                                                                                    |

## 5. Deviations from the plan and the design

Each is recorded in its ADR's "As built" section.

- **Gemini CLI** could not be added (the finding); **Antigravity** joined in its place at the
  owner's direction (ADR-082).
- **No older OpenAI model** is added: every one was refused on the owner's ChatGPT sign-in.
  Codex's newest model is added instead (ADR-081, as built).
- **Every model list says who made each model,** on every AI tool, as the owner's answer to
  choice 1 says (ADR-081 §6 limited the menus to AI tools that run other companies' models).
- **"Never use" and a model whose maker is not known:** skipped when a rule names any AI company,
  carrying choice 3 over (ADR-081, as built). The owner can ask for it the other way.
- **Antigravity's default model:** who made it is not known (ADR-082 §5, amending ADR-081 §2).
- **Adapter contract:** an AI tool may have a settings folder of its own (`own_home`), may wrap its
  prompt (`TurnParser::input`), and may sign in by starting on its own (ADR-082 §7).
- **Numbers:** ADR-080 to ADR-082; no new Ledger layout; no new desktop command.

## 6. Defects found and fixed during Phase 16 Wave 1

A review of four areas (Antigravity's safety; who made each model and review by maker; the
desktop screens and their tests; the records and guides), each finding checked by a second
reviewer. Of 35 findings, the second reviewers confirmed 23 as described and 12 as real but
smaller; none was rejected. Every one is fixed with a test, or recorded as a limit of the design.

**Antigravity**

- The sign-in check took any model that is not a Gemini model as a Google sign-in. Now only a
  model Plenipo's own list says another company made counts; anything else is not confirmed and
  never used. Test `a_google_sign_in_is_recognized_only_by_another_companys_model`.
- Every tool error counted as "refused". Now only a refused permission does; a tool that failed
  for another reason (it may have run) stops the task, and so does a step of an unknown kind that
  names a tool. Test `a_tool_that_failed_for_another_reason_or_under_another_kind_stops_the_task`.
- The strict-permissions check ran only when `init` came. Now nothing is accepted before it: a step
  or a finished answer first stops the task. Test
  `nothing_is_accepted_before_it_says_it_read_plenipos_settings`.
- The settings file was replaced on every run, which Windows can refuse while another run reads it.
  It is now written exactly as Antigravity writes it back, only when it differs, with a short
  retry. Contract test: settings changed by hand are put back before a check and before a task,
  and settings that cannot be written refuse the task.
- Its answer is kept to the result limit while it streams. Test
  `a_very_long_answer_is_kept_to_the_result_limit`.
- The fallback folder, when the app data folder cannot be found, now has a name only Plenipo uses.
- **Not changed, recorded as limits (ADR-082, Consequences):** `--disable-slash-commands` would also
  turn read-only mode off, so a task starting with `/` fails with Antigravity's own message; what
  the owner sets up inside Antigravity from its Sign in tab stays in Plenipo's folder for it; its
  own web search is not covered by any permission rule found, and a search that runs stops the task
  afterwards (for the owner's check, section 7).

**Who made each model**

- "Never use" let a model whose maker is not known through. Now it plays safe (section 5). Tests
  in the router's `a_company_never_to_use_is_skipped_as_the_maker_too` and
  `a_fixed_model_is_refused_by_who_made_it`.
- "Prefer a different company" did not try such a model last. Now: another company, then the same
  one, then not known. Test in `a_model_whose_maker_is_not_known_plays_safe`.
- The reasons on screen said "maker", repeated themselves, and dropped the known company when
  known and unknown work were reviewed together. Now plain words, with every company named; a
  test checks no reason says "maker".
- A maker attached to a model a tool reported could be trusted. Now only Plenipo's checked list
  says who made a model (`makers.rs` test).
- A Ledger read error made the work under review count as the AI tool's default model. Now its
  maker is not known on an AI tool that runs other companies' models (router test with
  `WorkDoneBy::unread`). New Liaison test for work passed on by its task ID.
- Ollama states its default model's maker in two places; a test keeps them the same.

**Screens and tests**

- A model whose maker is not known read "made by Not known" in the menus. Now "who made it is not
  known".
- The Models tab said who made each model only on Ollama and Antigravity. Now on every AI tool, as
  the owner's answer to choice 1 says; the menus too.
- The group heading used a column-group header in a row group. Now a row-group header, and the
  tests also check the visible heading.
- A test fixture, a CSS weight token, and a note about where the Antigravity stand-in keeps its
  state in the end-to-end tests.

**Records and guides**

- The checklist and the check page still said nothing was built and waited for E9; the ADRs'
  statuses; the owner's paid-credits screenshots and the Claude results (Part A) were missing from
  the evidence; ADR-082 did not say it changes ADR-081 §2; how Antigravity rewrites its settings
  was said too broadly; `/exit` was not shown (its built-in help says "`/exit` or `/quit`"); a few
  ADRs were cited by number only; the FAQ said "five providers"; the plan's Wave 1 list; the guide
  for adding an AI tool. All updated.

## 7. Left for the owner (on Windows)

1. **Accept or change ADR-082 (Antigravity as an AI tool).** Accepting it means keeping
   Antigravity as built, with its limits: text-only workers, its web search stopped after the fact
   if it runs, and anything set up in its Sign in tab kept in Plenipo's folder for it.
2. **Try Wave 1 on your PC:**
   - Settings → AI models → **Your models**: switch **Group by** between **Who made it** and **AI
     tool**; close and reopen Plenipo; it stays as you left it.
   - A reviewer role set to **require a different AI company**, reviewing work done on Ollama's
     gpt-oss: it should not pick Codex (both OpenAI).
   - A position on an exact Claude version (for example `claude-sonnet-5-5`): one task.
   - **Antigravity:** its card shows **Ready** and **Google sign-in**; one task; the same
     conversation again; **Cancel** during a longer task; a named model (for example
     `gemini-3.1-pro-high`); sign out of Google in Antigravity and check it says signed out; and one
     task asking it to "search the web for today's weather" — Plenipo should stop it, or it should
     answer without searching. Tell me which.
3. **Part D (Ollama)** when your paid plan starts, for the follow-up.
