# Phase 16 — Acceptance Report (Wave 2)

|              |                                                                                                                                                                                                                                                                                                                                                                            |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 16 — Every AI Model Worth Having, **Wave 2** ("one AI tool, one decision record each")                                                                                                                                                                                                                                                                                     |
| **Branch**   | `claude/laughing-archimedes-1s9onb` ([PR #109](https://github.com/Seckcey/plenipo/pull/109))                                                                                                                                                                                                                                                                               |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace`, `pnpm bindings` (no diff), and the end-to-end tests for the AI tools page and AI workers against the release build (section 3). GitHub CI on PR #109, Windows included.                                                                                                 |
| **Date**     | 2026-09-30 (Pacific time)                                                                                                                                                                                                                                                                                                                                                  |
| **Result**   | GitHub Copilot built (ADR-083, proposed, built with the owner's three answers); Cursor's agent a written finding (ADR-084, accepted). Every Wave 2 test the owner listed passes. Version **1.15.0**. A review of four areas, each finding checked by a second reviewer, is done (section 5). The walk-through with the real Copilot on Windows is the owner's (section 6). |

Screenshots (from the end-to-end run in the real app, with the stand-in AI tools):

- **GitHub Copilot:** [its card when GitHub may charge for extra use](evidence/phase-16-wave-2/ai-tools-copilot-paid-extra-use.png) ·
  [its Models tab](evidence/phase-16-wave-2/ai-tools-copilot-models.png) ·
  [a task on it, text only](evidence/phase-16-wave-2/worker-result-copilot.png)
- **Every AI tool, seven of them:** [Workers](evidence/phase-16-wave-2/agent-runtimes.png) ·
  [the AI tools page](evidence/phase-16-wave-2/ai-tools-page.png)
- **The owner's check on Windows:** [what it showed](evidence/phase-16-wave-2/owner-check/README.md)

## 1. What Wave 2 set out to do → result

| #   | Wave 2 (ROLLOUT_PLAN.md)                                                       | Result      | Evidence                                                                                                                                                                                                                                           |
| --- | ------------------------------------------------------------------------------ | ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Cursor's agent (its own models plus Anthropic's, OpenAI's, Google's, xAI's, …) | **Finding** | [ai-tools-cursor-finding.md](ai-tools-cursor-finding.md) and ADR-084 (Cursor's agent waits): nothing a program can run says whether Cursor may charge for on-demand use.                                                                           |
| 2   | GitHub Copilot, second try, through its `--headless --stdio` mode              | **Done**    | ADR-083 (GitHub Copilot as an AI tool, checked before every task): the adapter, its stand-in, the whole contract suite, screen tests, and end-to-end tests. Step 0 on the owner's PC ([evidence](evidence/phase-16-wave-2/owner-check/README.md)). |

## 2. The owner's choices → as built

| Choice                          | Answer                             | As built                                                                                                                         |
| ------------------------------- | ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| 1. Cursor                       | As recommended ("Grok is cursor.") | A written finding and ADR-084; xAI's models stay on Grok.                                                                        |
| 2. Which Copilot sign-in counts | "accept either"                    | **Copilot sign-in** (`user`) and **GitHub CLI sign-in** (`gh-cli`) both count; a token, a key, or an unknown sign-in never does. |
| 3. What Copilot workers do      | As recommended: text answers only  | All of Copilot's own tools off; a tool that runs anyway, or a model billed per use, stops the task; `accepts_tools: false`.      |

## 3. Tests → evidence

| Test (the owner's list)                                                            | Evidence                                                                                                                                                                                                                                                                                                                        |
| ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Each tool passes the whole contract suite with its own persona.                    | `crates/runtime/tests/contract.rs`: every test runs for Copilot's `copilot` persona (18 tests), plus `copilot_is_checked_before_every_task_with_a_settings_folder_of_its_own`: both sign-ins ready, paid extra use refused before the next task without a re-check, its own tool or a model billed per use stops a task.        |
| The contract suite still refuses key variables for every subscription AI tool.     | `no_api_key_or_cloud_variables_are_passed` (all seven tools); `sign_in_check_tells_a_subscription_from_an_api_key`; `agents.rs` `api_key_and_cloud_sign_ins_are_refused` now includes Copilot signed in with a token.                                                                                                           |
| With the paid switch off, nothing changes.                                         | `ai_tools.rs` `the_payment_switch_cannot_be_turned_to_a_paid_key_and_plans_come_only_as_reported` refuses a paid key for Copilot too; `aiTools.test.tsx` "the paid-key switch is locked…" (seven switches) and "with its paid-key switch locked like every tool's…". Paid extra use is never "ready" (unit and contract tests). |
| Cross-company review counts each new tool's models by who made them.               | Router `copilots_work_counts_by_who_made_it_and_auto_plays_safe`: Copilot's default (Auto) and the model Auto chose are "not known", so a must-be-different review never picks it or counts its work as another company's; "never use" skips it.                                                                                |
| Vitest for the screens.                                                            | `aiTools.test.tsx`: Copilot's sign-in words, no sign-out, the reason for paid extra use on the card, its Models tab, and when a card shows a reason at all.                                                                                                                                                                     |
| End-to-end tests in the real app, with screenshots in `evidence/phase-16-wave-2/`. | `ai-tools.e2e.mjs` "shows GitHub Copilot checked before every task, and why paid extra use stops it (ADR-083)"; `agents.e2e.mjs` "launches a GitHub Copilot task, text only, in Plenipo's own settings folder for it (ADR-083)". The screenshots above.                                                                         |
| Unit tests from the owner's recorded output.                                       | `copilot.rs` (16): the check (both sign-ins, tokens and keys, paid extra use on any allowance, no answer), a task, a continued conversation, a tool refused or run, a model billed per use, errors by kind, the pretend tool request in words, its models and plan left.                                                        |

## 4. The owner's rules → evidence

| Rule                                                                                               | Evidence                                                                                                                                                                     |
| -------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Never ask for passwords, keys, tokens, or secrets; never commit them.                              | No key is asked for or passed; the owner's check hid the account name, and the saved evidence has the user name removed; screenshots with billing were not kept.             |
| Anything that touches files, programs, the network, the browser, or the screen goes through Guard. | Copilot workers are conversation only (no Guard grants); its checks and tasks run under the supervisor like every AI tool; its npm release address is added to Guard's list. |
| Nothing loads code into Plenipo while it runs (ADR-014).                                           | The adapter is compiled in; Copilot is a separate supervised program.                                                                                                        |
| New desktop commands for the main window only.                                                     | No new desktop command: the AI tools page's existing commands (and their IPC tests) cover Copilot.                                                                           |
| Logs and diagnostics never hold secrets or anything typed in the terminal.                         | The account name from `auth.getStatus` is never kept (unit, contract, and `agents.rs` tests); the check's output is parsed, not logged.                                      |
| No model names in commits, branch names, or pull requests.                                         | Checked.                                                                                                                                                                     |
| Wave 2 does not change Connections or the Vault.                                                   | No file under `crates/capabilities/src/connections/`, `crates/guard/src/connections.rs`, `apps/desktop/src/settings/connections/`, or the Vault changed.                     |
| Plain words, ADRs named, 8 West Ventures, LLC credited.                                            | New word pairs in the word list; ADRs named wherever mentioned; the release notes credit 8 West Ventures, LLC.                                                               |

## 5. Review (four areas, each finding checked by a second reviewer)

Four reviewers looked at: billing and sign-in safety; the two-way check and the settings folder;
screens, words, and docs; and the test stand-in and tests. A second reviewer then checked each
finding and each fix.

**Fixed, each with a test:**

- A stray line from Copilot, or a message without its length, ended the check early
  (`framed_messages_are_found_past_stray_lines`).
- A tool failure whose words only looked like a refusal let the task go on; now only Copilot's
  own refusal ("Tool '…' does not exist." or code `denied`) does.
- An allowance reported as nothing did not block; now it counts as paid extra use on. A 402 or
  429 is a usage limit even when Copilot sorts it as a plain error.
- A refused task said "sign in" instead of the fix for paid extra use; now the check's reason
  comes first, then the sign-in steps, for every tool
  (`a_refusal_says_the_checks_own_reason_when_it_gave_one`).
- The card's reason showed for a tool given no tasks for another reason; now only when the
  sign-in check is why.
- Screen words: raw allowance names, "provider", a button name that differs between pages, an
  update line that assumed npm, and a plan-left line shown twice (code suggestions, which tasks
  never use, are left out).
- Tests that could pass without the feature: the check before every task is now proved without
  a re-check; stopped tasks are checked by their reason; the stand-in matches the recordings.

**Design limits (recorded in ADR-083 and here):**

- A model billed per use shows only in a failed call's report; in Copilot 1.0.89 it can be
  turned on only by a variable Plenipo never passes.
- A task is stopped after one of Copilot's own tools finishes, not when it starts: a refused tool
  also reports a start (as Antigravity, ADR-082).
- Copilot's hooks and add-on servers are not switched off by a setting: tasks run in Plenipo's
  own empty conversation folder, and Copilot's settings folder is Plenipo's own.
- `usageAllowedWithExhaustedQuota` is not read: it can mean free fallback models; paid use is
  `overageAllowedWithExhaustedQuota`.
- The settings folder's place follows the app's data folder, which is always a full path.

## 6. Checked by the owner on Windows

- GitHub Copilot through Plenipo: its card, **Sign in** (either sign-in), one task, the same
  conversation again, a cancel, and a refused check (paid extra use on, or signed out).
- Not part of this wave: the Wave 1 walk-through, and more Ollama cloud models.
