# Copilot: notes for the next decision record

These notes come from checking GitHub Copilot CLI 1.0.88 for Plenipo (see
[the finding](ai-tools-copilot-finding.md)). They collect what a decision record needs before
Copilot can be tried again. They cover three questions:

1. a two-way route for the check before each task;
2. paid extra requests;
3. which AI company made each model.

Nothing here is decided. Question 3 is a proposal waiting for the owner's answer. No shared code
has been changed.

Related records:

- ADR-007: how Plenipo runs Claude Code and Codex.
- ADR-011: how Plenipo picks each worker's AI model.
- ADR-014: adding AI tools ahead of Phase 15.
- ADR-015: running AI tools over ACP. It is proposed on `claude/ai-tools-grok` and not yet on
  `main`.

## 1. A two-way route for the check before each task

**What Copilot offers.** Copilot CLI speaks two protocols on standard input. Both answer signed
out:

| Route                       | How it starts                | Documented                                                                                                                                                | Signed out                                                                                                                                                                                                                       |
| --------------------------- | ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ACP (Agent Client Protocol) | `copilot --acp`              | Yes, in `--help`                                                                                                                                          | `initialize` works and lists one sign-in method ("Log in with Copilot CLI", which runs `copilot login`). `session/new` fails with `-32000 Authentication required`. Same shape as Grok's `grok agent stdio` (ADR-015's subject). |
| Copilot's own JSON-RPC      | `copilot --headless --stdio` | Not in `--help`. It is the connection GitHub's official `@github/copilot-sdk` (1.0.14, "programmatic control of GitHub Copilot CLI via JSON-RPC") starts. | `connect` and `status.get` work. `auth.getStatus` answers `{"isAuthenticated":false,"statusMessage":"Not authenticated"}`. `account.getQuota` and `models.list` answer "Not authenticated. Please authenticate first."           |

**What each route could do.** ACP has no call for the sign-in type or the allowance, so it does
not help with the checks Copilot fails. Copilot's JSON-RPC has three calls that do:

- **`auth.getStatus`**: `isAuthenticated`, `authType` (`user`, `gh-cli`, `env`, `token`,
  `api-key`, `hmac`), `host`, `login`, and `statusMessage`. Plenipo would keep the type and
  drop `login` (the account name).
- **`account.getQuota`**: one snapshot per allowance (for example `premium_interactions`, `chat`).
  Each has `entitlementRequests`, `usedRequests`, `remainingPercentage`, `overage`,
  `usageAllowedWithExhaustedQuota`, `overageAllowedWithExhaustedQuota`, and `resetDate`.
- **`models.list`**: every model the account can use, with its display name, the effort levels it
  takes (`supportedReasoningEfforts`, `defaultReasoningEffort`), its cost (`billing.multiplier`),
  and its policy state. There is no field for the model's maker.

**Suggested shape.** Tasks stay as they are under ADR-007: one program per task, prompt on
standard input, JSON lines out. Only the check before each task changes. Plenipo would start
`copilot --headless --stdio`, ask `connect`, `auth.getStatus`, and `account.getQuota`, wait for
every answer, then close standard input.

It must wait. Closing standard input early made the CLI exit and sometimes drop an answer
(2 of 6 runs; `evidence/ai-tools-copilot/headless-rpc-close-early.txt`). Today's contract runs a
status command and only reads its output (`auth_args` and `parse_auth` in
`crates/runtime/src/agent/adapter.rs`), so this is a contract change. It could be one optional
trait method, "the requests to send to the status command", plus a probe runner that keeps
standard input open until each request has an answer or the time limit passes.

**How the check would sort the answers** into today's `AuthState`s:

| Answer                                                            | State                                     |
| ----------------------------------------------------------------- | ----------------------------------------- |
| `isAuthenticated: false`                                          | `SignedOut`                               |
| `authType` `api-key` or `hmac`                                    | `ApiKey` (never ready)                    |
| `authType` `env` or `token` (a token variable reached the CLI)    | never ready; Plenipo never passes one     |
| `authType` `gh-cli` (the GitHub CLI's sign-in, not Copilot's own) | never ready; sign in with `copilot login` |
| `authType` `user`, and every allowance has paid extra usage off   | `Subscription`                            |
| `authType` `user`, and any allowance has paid extra usage on      | never ready; see §2                       |
| anything else, a timeout, or an error                             | `Unknown`                                 |

Only `user` is Copilot's own stored sign-in, which is the only one the owner allows.

**Open points for the owner's check** ([steps](ai-tools-copilot-owner-check.md)):

- the real `authType` after `copilot login`;
- which allowances `account.getQuota` lists;
- whether `overageAllowedWithExhaustedQuota` follows the GitHub budget setting.

## 2. Paid extra requests

What was found (details and evidence in the finding):

- Copilot plans include a monthly allowance, counted in premium requests or, on the newer
  billing, AI credits.
- Past it, GitHub charges for extra use if the account or its organization has a budget for it.
  The server decides; the CLI does not ask first.
- When paid extra use is off, a request past the allowance is refused. The CLI reports it as a
  `session.error` with `errorType: "quota"` (`quota_exceeded`, `session_quota_exceeded`,
  `billing_not_configured`) and exits 1.
- Being rate-limited is reported as `errorType: "rate_limit"` (`user_weekly_rate_limited`,
  `user_model_rate_limited`, …). The CLI can offer to switch to "Auto" model routing after a
  rate limit. That is off unless `continueOnAutoMode` is set.
- The result line of each task reports `usage.premiumRequests`.

**Proposed rules** for the decision record:

1. **Never ready while paid extra use is on.** The check before each task (§1) refuses to start
   a Copilot task while any allowance has `overageAllowedWithExhaustedQuota: true`. The on-screen
   message says, in plain words, to set the Copilot budget for extra use to $0 in GitHub and
   choose Re-check. With extra use off, GitHub itself refuses anything past the allowance, so
   Plenipo can never run up charges.
2. **An allowance used up is a usage limit.** The parser reads `session.error` by `errorType`
   rather than by wording: `quota` and `rate_limit` become a usage limit (`UsageLimited`), and
   `authentication` becomes "sign in again" (`AuthRequired`). The existing Router rules then
   apply: wait, or the next choice.
3. **No switch to Auto.** Plenipo never sets `continueOnAutoMode`, and one-task mode cannot
   approve the switch.
4. **Not a guard:** `--max-ai-credits`. It is a soft cap on all use, included or paid, and one
   request can go past it.

## 3. Which AI company made each model (proposal, waiting for the owner)

**The problem.** Plenipo treats an AI tool's company as the maker of all its models: Claude Code
means Anthropic, Codex means OpenAI. Two rules depend on it:

- "Never use these AI companies";
- "Reviews by a different AI company".

Copilot breaks that. Its Claude models are Anthropic's, its GPT models are OpenAI's, and it also
offers Google, xAI, Moonshot, and Microsoft models. ADR-014 says such a branch settles how the
Router counts this in its own decision record.

**How the code works today:**

- **The one place a model's company is decided:** `route()` in `crates/router/src/engine.rs:126`.
  It maps a model to its AI tool (`ModelInfo.runtime_id`), then to that tool's single company
  (`AgentRuntimeInfo.provider`, from `RuntimeAdapter::provider()`).
- **"Never use":** `engine.rs:221` compares the tool's company. The choices on screen
  (`companies()` in `apps/desktop/src/routing/format.ts:153`) list only the tools' companies.
- **"Reviews by a different AI company":** reviewed work is known only by its AI tool's ID. The
  ID is kept in `task.metadata.runtimeId`, and Liaison's `reviewed_work()`
  (`crates/liaison/src/service.rs:737`) reads it. The engine then looks up that tool's company
  (`engine.rs:157-180`, `:244-248`).
- **"Wait when a usage limit is reached":** `engine.rs:251-261` compares companies.
- **Models:** `KnownModel` (`crates/runtime/src/agent/dto.rs:102`) and the Router's `ModelInfo`
  (`crates/router/src/dto.rs:90`) have no maker field.
- **Records:** Ledger records keep the tool's company (`executions.provider`,
  `runtime_sessions.provider`, `agent_instances.runtime_provider`), but routing never reads them
  back.

**Proposal:**

1. **Each model can name its maker.** `KnownModel` gets an optional maker (a company ID). A tool
   whose models all come from its own company leaves it empty, so Claude Code and Codex do not
   change. Copilot's adapter would name the maker of each model it lists, checked by hand like
   the rest of the list. Copilot's own model list has no maker field.
2. **One list of AI companies.** A short shared list of company IDs and names gives each maker
   one name everywhere: `anthropic` Anthropic, `openai` OpenAI, `google` Google, `xai` xAI,
   `moonshot` Moonshot AI, `microsoft` Microsoft, `github` GitHub. Each tool's own `provider()`
   stays as it is.
3. **The Router's company for a model** is:
   - its maker, if the owner's model entry or the tool's own list names one;
   - otherwise, the tool's company, for tools that make all their own models;
   - otherwise, **unknown**, on a tool that runs several companies' models.

   An unknown maker is never chosen under "Never use" or "Only a different AI company", and
   comes last under "Prefer a different AI company". When the owner adds a model by hand in
   Settings → AI models, they can choose its maker. `ModelInfo` gets the same optional field.

4. **Reviews compare makers.** The task records the maker of the model that did the work, next
   to the AI tool's ID. It comes from the route choice's company, which is the maker under
   point 3. The "different AI company" test compares makers. Tasks recorded before this change
   have no maker, so they fall back to the tool's company, as today.
5. **Usage limits stay with the AI tool's account.** A limit is about whose allowance ran out:
   GitHub's for Copilot, even for a Claude model. So "wait, never move the work to another AI
   company" keeps comparing the tools' own companies. **Question for the owner:** should
   waiting instead keep work with the same maker? That would let a Claude model on Copilot move
   to Claude Code when Copilot's allowance runs out.
6. **Records keep the tool's company**, because that is who bills. Screens can show both, for
   example "Claude Sonnet 5 (Anthropic) through GitHub Copilot".

**What changes in shared code:**

- `crates/runtime/src/agent/dto.rs`: `KnownModel.maker`, and the company list.
- `crates/router`:
  - `ModelInfo.maker`;
  - the company mapping in `engine.rs` (`route()`, the reason sentences);
  - `reviewed` becomes makers;
  - `Planner::fixed()`;
  - `snapshot()` lists every company, makers included;
  - `check_policy()` accepts any listed company.
- `crates/liaison`: `reviewed_work()` reads the recorded maker.
- `crates/workforce`: the route decision stored with the task carries the maker; the
  `reviewed` values are passed on.
- `apps/desktop`: the "Never use" choices list makers too; the model menus can show a model's
  maker; Settings → AI models can set the maker of a model the owner adds by hand.
- `packages/types/src/generated`: `KnownModel`, `ModelInfo`, and `ToolInfo` (`pnpm bindings`).
- The contract suite checks that each maker is a listed company.
- Router and Workforce tests cover a tool that runs two companies' models: "Never use", a review
  of its Claude work picking a non-Anthropic model, and an unknown maker.

**Where it belongs.** A new decision record, "Which AI company made each model". It amends
ADR-011 (how Plenipo picks each worker's AI model) and completes ADR-014's note on tools that
run other companies' models. It is only needed once a multi-company AI tool (so far, only
Copilot) passes the bar.
