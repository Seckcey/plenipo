# A live chat with each agent, light by default, and the chain of command — Acceptance Report

|              |                                                                                                                                                                                                                                                                                    |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Scope**    | A live chat with each agent (ADR-200), light by default with Settings → Safety and Plenipo's own folder (ADR-201), and the chain of command (ADR-202), asked for by the owner on 2026-10-03                                                                                        |
| **Branch**   | `claude/fervent-dirac-h0fove` ([PR #165](https://github.com/Seckcey/plenipo/pull/165))                                                                                                                                                                                             |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt`, `cargo clippy`, `cargo test` (one package at a time, as a non-administrator user; see §3), and `pnpm bindings` with no change. The chat drawn in a real Chromium with sample data for the screenshots. GitHub CI on the pull request. |
| **Date**     | 2026-10-03                                                                                                                                                                                                                                                                         |
| **Result**   | Every item in the [checklist](chat-light-chain-checklist.md) is built and tested, and released in v1.23.0. The owner's Windows check is in the checklist.                                                                                                                          |

Screenshots (the chat drawn in Chromium with sample data, from this branch):

- [a chat: the answer, what the agent is doing now with its timer, Files saved, and the Tasks and Chain of command beside it](evidence/chat-light-chain/chat.png)
- [Side by side: three chats at once](evidence/chat-light-chain/side-by-side.png)
- [the light theme](evidence/chat-light-chain/chat-light-theme.png)
- [Settings → Safety](evidence/chat-light-chain/settings-safety.png)

## 1. What the owner asked for → evidence

| Asked for                                                          | Built                                                                                                                                                                          | Evidence                                                                                                                                                                                          |
| ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A live chat with each agent, from the org canvas                   | A **Chat** panel (dock or its own window), a tab for each agent, **Side by side** for four; Chat on the map's tiles, in the Inspector, and in the top bar                      | `Chat.test.tsx`, `tabs.test.ts`, `layout.test.ts`, the screenshots                                                                                                                                |
| Chat windows that work like Claude Code, streaming thoughts & work | Words stream as written; thinking and each run of tool calls are one line that opens; what it is doing now with a timer; a message sent while busy waits its turn; **Stop**    | `Chat.test.tsx` ("streams the answer…", "holds a message…", "takes back a waiting message, and Stop…"), `model.test.ts`, `markdown.test.ts`, `Markdown.test.tsx`                                  |
| Not a bare "thinking"; it took three minutes                       | Claude Code's own retries and waits shown in plain words ("Claude's servers are busy. Trying again in 4 s (try 2 of 10)."), Claude's thinking shown (2.1.288 and later)        | the runtime's Claude Code tests (`api_retry`, `requesting`, thinking, `--thinking-display`), `Chat.test.tsx`                                                                                      |
| Lighter permissions; warnings in Settings                          | **Settings → Safety**: Light (the start), Careful, Strict; leads start with **Everyday work**; the safety rules stay on every setting                                          | the Guard tests (Light, Careful, Strict, an older document, the one-time move), `SafetySettings.test.tsx`, `safety_starts_light_and_the_owner_can_turn_it_up`                                     |
| "This session cannot save files or run programs" — the opposite    | Work with no project folder is done in `Documents\Plenipo\<organization>\<position or project>`; the agent is told its folder and to say where it saved a file                 | `plenipo_starts_light_an_agent_saves_a_file_and_runs_a_program`, `work_that_belongs_to_no_project_is_done_in_a_folder_of_plenipos_own`, `a_project_without_a_folder_works_in_plenipos_own_folder` |
| "I don't know where an agent's output went"                        | **Files saved** under each answer, the folder it is in, and **Open folder**, found from Plenipo's own record (the page names only the task)                                    | `the_work_folder_comes_from_plenipos_own_record`, the broker tests' `work_folder` checks, `Chat.test.tsx`                                                                                         |
| Orders go down; reports come up one level at a time                | `chain.order` names the skipped leads; `chain.report` passes the result up, nearest first; a lead's agent hears the news once; an on-call worker's order goes through its lead | `an_order_that_skips_a_level_is_told_to_the_lead_and_reported_back_up`, `an_order_for_an_on_call_position_goes_through_its_lead`, `the_chain_of_command_is_read_by_position`                      |
| Research on how a chain of command reports                         | Doctrine (FM 6-0, the Navy's War Instructions), the Incident Command System's unit log and status summary, and RACI, summed up in ADR-202's Context                            | ADR-202                                                                                                                                                                                           |
| (safety) Claude Code's own "skip permissions" mode                 | **Not used**: Guard stays the one that decides                                                                                                                                 | ADR-201's Alternatives                                                                                                                                                                            |

## 2. Found and fixed while building

- **A lead in its team's project folder** briefly became that folder's one writer with Everyday
  work: the Developer it handed the change to got a second working copy, and the owner's own saves
  waited. A lead now reads there and hands each change on; Everyday work applies to its own work
  in Plenipo's own folder (`a_lead_reads_in_its_teams_project_folder_and_hands_changes_on`; the
  Development department's tests pass unchanged).
- **News that repeated the owner's words** carried a hand-off mark, and the lead's agent acted on
  it. News now repeats other people's words cut short and without the marks instructions are
  written with (`news_repeats_others_words_without_instruction_marks`).
- **The chat's message box** shared a style name with Phase 24's Messages page; it is
  `chat-composer` now.

## 3. How the Rust tests were run here

This sandbox runs as administrator, and Plenipo refuses to start AI tools as administrator. The
test programs were built with `cargo test --no-run` and run one package at a time as an ordinary
user. Three things failed only because of that, and were checked another way:

- The type-export tests could not write `packages/types/src/generated` as that user;
  `pnpm bindings`, run normally, writes them, with no change left over.
- One supervisor test expects `CARGO_PKG_NAME`, which `cargo test` sets; with it set, all 29 pass.
- The sandbox's own Git settings sign every commit with a program that fails inside the tests;
  with a plain Git setting, the git-tool and Development department tests all pass.

## 4. Not done here

- **Text-only AI tools** (Ollama, OpenRouter, direct keys, GitHub Copilot, Antigravity) still
  cannot save files or run programs. That is Phase 16's parked Wave 4 (ADR-131, ADR-132); only the
  owner can schedule it. The chat says so when it applies.
- **A faster start for each message.** Each message still starts its own AI tool program
  (ADR-007). Keeping one Claude Code program running for a whole conversation would start each
  message faster; it changes how every AI tool runs and is its own decision.
- **Opening links in the browser.** A link an agent wrote copies its address instead.
