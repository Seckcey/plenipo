# Phase 16 — Acceptance Report (Wave 3)

|              |                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Phase**    | 16 — Every AI Model Worth Having, **Wave 3** ("spending caps first, then paid routes")                                                                                                                                                                                                                                                                                                                                                     |
| **Branches** | `claude/phase-16-wave-3-caps` ([PR #111](https://github.com/Seckcey/plenipo/pull/111)), `claude/phase-16-wave-3-keys` ([PR #113](https://github.com/Seckcey/plenipo/pull/113)), `claude/phase-16-wave-3-direct` (part 3); the Windows fixes first in [PR #112](https://github.com/Seckcey/plenipo/pull/112)                                                                                                                                |
| **Verified** | Locally on Windows for each part: `pnpm check`, `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace`, `pnpm bindings` (no diff), and `pnpm docs:check`. GitHub CI on each pull request, Windows and the end-to-end tests included.                                                                                                                                                                                            |
| **Date**     | 2026-09-30 (Pacific time)                                                                                                                                                                                                                                                                                                                                                                                                                  |
| **Result**   | Spending caps, the paid-keys switch, keys in the Vault, paid routes, OpenRouter, and ten AI companies' own services built (ADR-085, ADR-086, ADR-087, all proposed, built with the owner's choices). Version **1.17.0** (Phase 21 merged first, as 1.16.0). A review of each part across several areas, each finding checked by a second reviewer, is done (section 5). The checks with the owner's real keys are the owner's (section 6). |

## 1. What Wave 3 set out to do → result

| #   | Wave 3 (ROLLOUT_PLAN.md)                                                                         | Result      | Evidence                                                                                                                                         |
| --- | ------------------------------------------------------------------------------------------------ | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | Spending caps for the business, a department, and a position; monthly; warning at 80%; hard stop | **Done**    | Part 1 (PR #111): `crates/ledger/src/spending.rs`, migration 0012, Settings → Spending caps, the banner and the notice; ADR-085 §2.              |
| 2   | Pricing and recording of every paid task                                                         | **Done**    | `crates/runtime/src/pricing.rs`; the `spending` table; the gate (set aside, then settle); ADR-085 §3–§4.                                         |
| 3   | "Let workers use paid AI keys", off by default                                                   | **Done**    | `Switches.paid_ai_keys`; Settings → Switches.                                                                                                    |
| 4   | Paid keys in the Vault, reaching only their own AI tool                                          | **Done**    | Part 2 (PR #113): `crates/capabilities/src/paid/`; the key on the helper's standard input only; the secret filter; ADR-085 §5.                   |
| 5   | More than one route to a model, in the owner's order, with fallback                              | **Done**    | `crates/router/src/engine.rs`: a paid route only where listed, skipped when not priced, over its cap, or without its key; the same model linked. |
| 6   | OpenRouter through a Plenipo helper                                                              | **Done**    | `crates/capabilities/src/paid/helper.rs`, `crates/runtime/src/agent/paid.rs`; ADR-086.                                                           |
| 7   | Direct keys (widened by the owner to every AI company whose models take one)                     | **Done**    | Part 3: `crates/runtime/src/agent/direct.rs`, `crates/guard/src/paid.rs`; ten companies; NVIDIA has no row (no price per use); ADR-087.          |
| 8   | Wave 1's leftover: Ollama's paid-plan models                                                     | **Waiting** | The paid Ollama plan starts 2026-10-01; its models are checked then.                                                                             |

## 2. The owner's choices → as built

| Choice                    | Answer                                 | As built                                                                                                |
| ------------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| 1. Which cap first        | The business's                         | No key saved and no paid task without it; it cannot be removed while a key is saved.                    |
| 2. The month              | Pacific calendar month                 | Starts over on the 1st at midnight, Pacific time, daylight saving included.                             |
| 3. The hard stop          | Never goes over                        | The most a task could cost is set aside first, in one Ledger transaction.                               |
| 4. No known price         | No paid key                            | "Not priced yet": skipped by the Router, refused by the gate, never counted as zero.                    |
| 5. Direct keys            | Through Plenipo's own helper           | One helper for OpenRouter and every company, speaking OpenAI-style chat or Anthropic's own messages.    |
| 6. What paid workers do   | Text only                              | `accepts_tools: false`; the Router's reason says "A worker on it answers in text only."                 |
| 7. OpenRouter's models    | A short checked list, plus any by name | Seven listed; any other by its exact name; prices from OpenRouter's list before each task.              |
| 8. Privacy on OpenRouter  | No limits                              | No provider limits sent.                                                                                |
| 9. Wave 1's leftover      | Folded in                              | Waiting for the paid Ollama plan (2026-10-01).                                                          |
| 10. Windows fixes         | First, in a small pull request         | PR #112, with clippy added to the Windows job.                                                          |
| 11. How the work is split | Three pull requests                    | Caps → keys, routes, and OpenRouter → direct keys and the version.                                      |
| 12. The version           | Set by the last pull request           | 1.17.0.                                                                                                 |
| 13. Merging and releasing | Allowed                                | Merged by the builder; the release started once every check passed (the owner approves the signed run). |
| 14. Which companies       | Every one whose models take a key      | Anthropic, OpenAI, xAI, Moonshot AI, Google, DeepSeek, Z.ai, MiniMax, Mistral, Alibaba Cloud.           |

## 3. Tests → evidence

| Test (the checklist)                                                                  | Evidence                                                                                                                                         |
| ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Money and Pacific months; 80% warning; hard stop; business, department, position caps | `crates/ledger/src/spending.rs` tests.                                                                                                           |
| A task that could pass a cap is not started; restarts count once, at the most         | `spending.rs` (`recover_spending`), `crates/capabilities/tests/ai_tools.rs`.                                                                     |
| "Not priced yet" is never zero                                                        | `spending.rs`, `paid.rs` (`a_bill_without_counts_is_not_priced…`), the Router (`a_paid_route_over_its_cap_or_not_priced_is_skipped`).            |
| No key saved without the switch and the business cap                                  | `a_paid_key_is_kept_only_in_the_vault_and_only_with_the_switch_and_the_business_cap`; the IPC tests.                                             |
| No key in the Ledger, a log, a task's activity, or the diagnostics file               | The same test; `errors_never_carry_the_key`; the contract suite's `last-env.txt` and arguments checks.                                           |
| Route fallback; the owner's example (Kimi usage-limited → Kimi K3 on Ollama)          | `the_owners_example_kimi_runs_out_and_the_ollama_plan_carries_the_work`, `a_paid_route_runs_next_and_the_reason_says_it_costs_money`.            |
| The contract suite for every AI tool and every company                                | `crates/runtime/tests/contract.rs`: a task on each of the 18 AI tools, each paid one with its key; `a_paid_ai_tool_uses_its_key_only_on_stdin…`. |
| Each company asked in its own words; each price from its own page                     | `helper.rs` (`each_company_is_asked_in_its_own_words`), `direct.rs` tests.                                                                       |
| Desktop commands main-window only                                                     | IPC tests for the spending and key commands (the sign window, another window, and web pages refused).                                            |
| Screens                                                                               | Vitest: the Spending caps page, the banner, the switch, the key form, prices, and "also on …".                                                   |

## 4. Deviations, each recorded in its ADR

- The key goes on the helper's standard input, not an environment variable (stricter than ADR-036
  §2.4; ADR-085 §5).
- NVIDIA has no row: its key comes with trial credits and no price per use (ADR-087 §6).
- Where a company's answer-length field does not limit the thinking (xAI) or does not say
  (Moonshot AI, Z.ai), each step sets aside the most the model can write (ADR-087 §3).
- Paid keys work in the first organization only for now (Phase 21, ADR-094; ADR-085 as built).
- A paid AI tool gets no model listed by itself in Settings → AI models (ADR-085 as built).

## 5. Review

Each part had its own review across several areas, each finding checked by a second reviewer, with
tables in [the checklist](phase-16-wave-3-checklist.md): part 1 (money and caps, security and
desktop commands, the Ledger's data, screens), part 2 (secrets and the Vault, money and caps, Guard
and the network, desktop commands and routes, screens and words), and part 3 (money and prices, the
network and secrets, the adapter and screens). The largest catches: the key check's price list cut
at 64 KB (no key could have been saved), OpenRouter routing to a dearer company, the owner's own
company keys on OpenRouter billing only its 5% fee, Alibaba Cloud's and xAI's thinking outside the
answer's limit, and Google's chat ignoring the key header.

## 6. The owner's checks on Windows

- Type the keys for Anthropic, OpenAI, xAI, and Moonshot AI into their cards; each should say
  "Your key works". With a small business cap, run one short task on each through a position's
  list, and see its cost on Settings → Spending caps.
- OpenRouter and the other companies when their keys arrive.
- The paid Ollama plan's models once the plan starts (2026-10-01).

## 7. After v1.17.0: the owner's changes (v1.18.1)

The owner tried v1.17.0 on 2026-09-30 and could not find where to add a key: the key boxes were on
the paid AI tools' cards, far down the page. The owner asked for three changes, built in v1.18.1
([ADR-085, Fix (v1.18.1)](../adr/ADR-085-paid-ai-keys-with-spending-caps.md#fix-v1181-2026-09-30)):

- **No spending cap is needed** to add a key or run paid work; the screen says a limit can be set
  in Spending caps and is not required. Every paid task is still priced and recorded.
- **A key box on every AI tool card**, saving the same AI company's key (or OpenRouter's for
  Ollama and GitHub Copilot), and the paid AI tools in their own part of the page.
- **Words that no longer fit**, found by a scan of the screens, the website, and the docs:
  "never asks for passwords or API keys", "No paid key works without the business's cap",
  "subscription sign-ins only", "Pay-per-use API billing: Off", and the preview pictures'
  "never API keys".

The owner's checks in section 6 now start from any AI tool's card, with or without a cap.
