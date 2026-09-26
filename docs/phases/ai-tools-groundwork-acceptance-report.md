# AI tools groundwork — Acceptance Report

|              |                                                                                                                                                       |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Track**    | AI tools: groundwork (`ROLLOUT_PLAN.md` Phase 15, adapter parts, pulled forward)                                                                      |
| **Branch**   | `claude/ai-tools` (draft pull request)                                                                                                                |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, `pnpm bindings`, full `pnpm e2e` against the release build. GitHub CI: see the pull request. |
| **Date**     | 2026-09-26                                                                                                                                            |
| **Result**   | **Ready for the owner.** Every deliverable is in place and the contract suite passes for Claude Code and Codex. ADR-014 is Proposed.                  |

This branch adds no AI tool. It gives the Gemini, Copilot, and Grok branches three things:

- one guide to follow;
- a contract suite that covers any adapter registered in `builtin_adapters()`;
- fake-CLI helpers that install any new persona without being edited.

Test totals: **421 Rust** (Linux; 414 before, plus the 7 contract tests) · **118 frontend** · **35 end-to-end** against the release
binary (unchanged in number: the end-to-end specs now install the fake AI tools through one
helper).

## 1. Deliverables → evidence

| Deliverable                              | Where                                                                                                                                                                                                                                                                                 |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Decision record                          | [ADR-014 (adding AI tools ahead of Phase 15)](../adr/ADR-014-adding-ai-tools.md), Proposed, in the [index](../adr/README.md). It covers why now, the bar, one branch per tool, findings, what each adapter provides, and what is out of scope.                                        |
| Documented adapter SDK/contract          | [`docs/development/adding-an-ai-tool.md`](../development/adding-an-ai-tool.md): step 0 (check the real CLI), then each part of `RuntimeAdapter` with Claude Code and Codex as examples, the fake persona, registration, the setup guide, a checklist to copy, and a finding template. |
| Model capability discovery contract      | `RuntimeAdapter::checked_version()` (Claude Code `2.1.283`, Codex `0.157.1`), and the rules for `effort_levels` and `known_models` (guide §7), checked by the suite.                                                                                                                  |
| Provider adapter contract suite          | [`crates/runtime/tests/contract.rs`](../../crates/runtime/tests/contract.rs): 7 tests, each looping over `builtin_adapters()` (§2).                                                                                                                                                   |
| Fake CLI ready for new personas          | `PERSONAS` table in `crates/runtime/src/bin/plenipo-fake-agent.rs`; `plenipo-fake-agent --personas` lists them.                                                                                                                                                                       |
| Test helpers ready for new personas      | `personas()` in the runtime, Liaison, and Workforce integration tests; `installFakeTools(home)` in `tests/e2e/lib/app.mjs`, used by the four end-to-end specs that run AI tools.                                                                                                      |
| IPC tests follow the registered AI tools | `tool_ids()` in `apps/desktop/src-tauri/src/lib.rs`: the AI tools overview, the Liaison destinations, and the organization's AI tools are compared with `builtin_adapters()`.                                                                                                         |

## 2. Contract suite → what it checks

Each test runs for every adapter in `builtin_adapters()` and names the adapter when it fails.

| Test                                                                 | Checks                                                                                                                                                                                                                                        |
| -------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `every_ai_tool_is_named_and_distinct`                                | ID, label, company ID and name, executable, and hints are filled in. IDs are lowercase letters, digits, and dashes. IDs, labels, and executables are unique, and each company has one name.                                                   |
| `known_models_are_valid_distinct_and_within_the_tools_effort_levels` | `checked_version()` is a CLI version. Effort levels are lowest first. Model names pass `validate_model`, are listed once, and have labels. Each model's levels are within the tool's.                                                         |
| `turn_arguments_carry_the_choices_and_no_secrets`                    | For every request shape (new, new with Plenipo's ID, resumed × default and each known model × default and each effort), no credential-bearing argument, and the model, effort, and session ID are all passed.                                 |
| `no_api_key_or_cloud_variables_are_passed`                           | No pass-through or fixed variable whose name suggests a key, token, secret, password, credential, cloud billing, or another endpoint.                                                                                                         |
| `parsers_ignore_malformed_lines_and_finish_with_one_result`          | HTML, broken JSON, a JSON array, an unknown event, and an over-long line are each ignored and counted, never stop the turn, and never crash. `finish()` gives one result for six ways a process ends, never "completed" without a completion. |
| `sign_in_check_tells_a_subscription_from_an_api_key`                 | Every adapter has a fake persona. On the fake CLI: subscription is ready; an API key is **never** ready; signed out is not ready.                                                                                                             |
| `prompts_go_on_stdin_and_limits_and_sign_in_errors_are_normalized`   | On the fake CLI: the prompt reaches the CLI on stdin (it is echoed back) and never appears in its arguments. A usage limit is `usageLimited` and an expired sign-in is `authRequired`.                                                        |

Checked against three deliberate breaks in the Codex adapter: an API-key variable passed through,
the resume ID dropped, and a model listed twice. Each one failed the matching test and was
reverted.

## 3. Shared files and Phase 7

Phase 7 (Guard, `claude/phase-7`, [PR #14](https://github.com/Seckcey/plenipo/pull/14)) is open
and changes the adapter files. This branch keeps its edits to those files small and away from the
lines Phase 7 changes:

| File                                                  | This branch                                                                                         | Phase 7 changes                                                  | Overlap                                                                          |
| ----------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `crates/runtime/src/agent/adapter.rs`                 | `Default` for `TurnRequest` and `ProviderSession`; `checked_version()` added after `capabilities()` | `tools` field on `TurnRequest`; `turn_env()` after `turn_args()` | None expected: different lines. `Default` keeps working with `tools: Option<…>`. |
| `crates/runtime/src/agent/claude_code.rs`, `codex.rs` | `checked_version()` after `capabilities()`                                                          | Tool server arguments, tests                                     | None expected                                                                    |
| `crates/runtime/src/bin/plenipo-fake-agent.rs`        | The top doc lines and `main()` (persona table)                                                      | Tools note, MCP client, markers (from line 24 on)                | None expected                                                                    |
| `docs/adr/README.md`                                  | Row 014                                                                                             | Row 013 (planned)                                                | **One-line conflict:** both add a row at the end. Keep both, 013 first.          |
| `apps/desktop/src-tauri/src/lib.rs`                   | Three test asserts and `tool_ids()`                                                                 | Not yet changed (Phase 7 still has to add its IPC commands)      | Possible, in the test module only                                                |
| `crates/capabilities/tests/broker.rs` (Phase 7 only)  | —                                                                                                   | Installs the fakes as `["claude", "codex"]`                      | No conflict, but it should switch to `personas()` so new tools reach it.         |

## 4. Security notes

- No behavior changes for the owner: the same two AI tools, the same arguments, and the same
  sign-in and billing rules. The additions are contract checks, test helpers, and docs.
- The contract suite makes the billing rules checkable for every future AI tool:
  - no credential-bearing argument or variable;
  - an API-key sign-in is never ready;
  - the prompt is only on stdin.
- The fake CLI's `--personas` answers only when the binary runs under its own name. It is a test
  binary and is never shipped.

## 5. Deviations from the plan

| Deviation                                                                                              | Why                                                                                             | Recorded |
| ------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------- | -------- |
| Phase 15's adapter contract, capability discovery contract, and contract suite now, before Phases 7–14 | The owner wants every frontier lab's models; the adapter contract has been stable since Phase 3 | ADR-014  |
| "AI company unique" is read as one name per company ID, not one AI tool per company                    | Two AI tools may come from the same company. The ID, label, and executable are unique.          | Guide §1 |

## 6. Owner items

| ID  | Item                                                                                                                                                    | Recommendation                                 |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| O1  | Accept ADR-014 (adding AI tools ahead of Phase 15): the five-point bar, one branch per tool, and a finding instead of a workaround.                     | Accept, so the tool branches can start step 0. |
| O2  | Step 0 on Windows for each tool: run the guide's real-CLI checks and paste the outputs into that tool's checklist. Remove account names and keys first. | One tool at a time, starting with Gemini.      |
| O3  | Merge order: this branch first, then Phase 7 whenever it is ready. Tool branches merge `main` after each.                                               | As in the checklist's order of work.           |

## 7. Verification

| Check                                                            | Result                                           |
| ---------------------------------------------------------------- | ------------------------------------------------ |
| `pnpm check` (versions, format, lint, typecheck, tests)          | Pass — 118 frontend tests                        |
| `cargo fmt --all -- --check`                                     | Pass                                             |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass                                             |
| `cargo test --workspace --locked`                                | Pass — 421 tests, including the 7 contract tests |
| `pnpm bindings`, then no diff in `packages/types/src/generated`  | Pass — no diff (no shared types changed)         |
| `pnpm e2e` against the release build (Linux, Xvfb)               | Pass — 35 of 35                                  |
| GitHub CI on the pull request                                    | Linked from the pull request                     |

## 8. Next

Once this merges, the Gemini, Copilot, and Grok branches merge `main` and start with step 0 of
the [guide](../development/adding-an-ai-tool.md). Each ends in an adapter with its own acceptance
report, or in a finding.
