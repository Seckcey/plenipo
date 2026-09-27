# AI tools: Ollama (cloud models) — Acceptance Report

|              |                                                                                                                                                               |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Track**    | AI tools: Ollama's cloud models, under ADR-014 (adding AI tools ahead of Phase 15) and ADR-017 (running Ollama's cloud models through its service on this PC) |
| **Branch**   | `claude/ai-tools-ollama` (pull request #21)                                                                                                                   |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, `pnpm bindings`, full `pnpm e2e` against the release build. GitHub CI. The owner's check on Windows. |
| **Date**     | 2026-09-27                                                                                                                                                    |
| **Result**   | **Accepted.** The owner checked the real Ollama 0.34.4 on Windows, signed in on the free plan: every check passed. The owner keeps the free plan for now.     |

## 1. How Ollama got here

1. **Step 0 on the real CLI** (2026-09-26). The owner has no dedicated graphics card, so Plenipo
   uses Ollama's **cloud models**, which run on Ollama's servers under the owner's ollama.com
   sign-in (`ollama signin`). `ollama run` gives only plain text; the Ollama service on
   `127.0.0.1:11434` gives structured, streamed answers, the sign-in and plan (`/api/me`), and
   no conversation memory. Evidence: [`crates/runtime/tests/fixtures/ollama-0.34.4/`](../../crates/runtime/tests/fixtures/ollama-0.34.4/README.md).
2. **ADR-017 (running Ollama's cloud models through its service on this PC)**, accepted by the
   owner on 2026-09-27: cloud models only, the owner's sign-in only, no API key, Plenipo keeps
   the conversation, and conversation only at first (Plenipo's tools come later, through Guard).
3. **The owner's models** were checked with `ollama show` and one message each, signed in on
   the free plan (2026-09-27).

## 2. What was built

| Deliverable                     | Where                                                                                                      |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Plenipo's Ollama helper         | `crates/runtime/src/agent/ollama/bridge.rs` (`plenipo-desktop --plenipo-ollama auth\|chat`)                |
| The Ollama adapter              | `crates/runtime/src/agent/ollama/mod.rs`                                                                   |
| Bridged AI tools in the runtime | `bridged()` and `accepts_tools()` on the adapter trait; `AgentConfig::bridge` (`service.rs`)               |
| Desktop wiring                  | Helper mode before any window opens (`main.rs`); the bridge points at Plenipo itself (`agent_host.rs`)     |
| Test double                     | The `ollama` persona in `plenipo-fake-agent` (its program and the helper)                                  |
| Tests                           | Helper against a stand-in service; parser; contract suite; runtime harness; desktop, workforce, end to end |
| Docs                            | ADR-017 (with "How it was built"), the checklist, setup guide, adding-an-AI-tool guide, fixtures           |

## 3. The owner's models

| Model                       | Thinking choices  | Free plan                                    |
| --------------------------- | ----------------- | -------------------------------------------- |
| `gpt-oss:120b-cloud`        | low, medium, high | Answered                                     |
| `nemotron-3-ultra:cloud`    | none (always on)  | Answered                                     |
| `kimi-k3:cloud`             | low, high, max    | `402 Payment Required`: listed "(paid plan)" |
| `deepseek-v4-pro:cloud`     | low, high, max    | `402 Payment Required`: listed "(paid plan)" |
| `deepseek-v4.1-flash:cloud` | low, high, max    | `402 Payment Required`: listed "(paid plan)" |
| `glm-5.3:cloud`             | low, high, max    | `402 Payment Required`: listed "(paid plan)" |
| `glm-5.3-flash:cloud`       | low, high, max    | `402 Payment Required`: listed "(paid plan)" |
| `minimax-m3:cloud`          | none listed       | `402 Payment Required`: listed "(paid plan)" |

A task on a paid-plan model ends with "This model needs a paid Ollama plan". Plenipo never
changes the plan.

## 4. The owner's check on Windows (2026-09-27)

| Check                                                                                   | Result |
| --------------------------------------------------------------------------------------- | ------ |
| The AI tools card: Ready, v0.34.4, "Ollama sign-in (free plan)"                         | Pass   |
| A task on gpt-oss 120B, then a follow-up that remembers the first question              | Pass   |
| A task on a paid-plan model ends with "This model needs a paid Ollama plan"             | Pass   |
| Signed out (`ollama signout`): not signed in, with the `ollama signin` hint; back after | Pass   |
| Ollama not running: the card says so ("Start Ollama"); Ready again once it starts       | Pass   |

## 5. Safety

- Plenipo talks only to `127.0.0.1:11434`, ignores `OLLAMA_HOST`, and passes no environment
  variables to the helper; it never uses or sets an Ollama API key.
- The sign-in check keeps the plan only, never the account's email or name.
- Ollama workers cannot read files or run programs: no tools and no permission grant.
- Conversation files are named by an ID Plenipo checks (letters, digits, dashes), in the
  session's own folder.

## 6. Open items

- **Tools for Ollama workers** (ADR-017 §4): a follow-up with its own decision record, comparing
  Plenipo's helper handling tool calls through Guard with running Claude Code or Codex on an
  Ollama model (as `ollama launch` does).
- **The free plan's usage-limit message**: recorded the first time it is seen.
- **A paid Ollama plan**: the six "(paid plan)" models need one; the owner keeps the free plan
  for now.
