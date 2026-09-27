# ADR-017: Running Ollama's cloud models through the Ollama service on this PC

- **Status:** Accepted (by the owner, 2026-09-27)
- **Date:** 2026-09-27
- **Phase:** 15 (adapter parts pulled forward, after v0.8.0)

> **On screen** (ADR-010, plain words and rank names): the owner sees an AI tool "Ollama" with
> its models, "Signed in (free plan)", and usage limits, never "HTTP", "/api/chat", or "service".

## Context

The owner wants Ollama's **cloud models**: models that run on Ollama's servers under the owner's
ollama.com account (`ollama signin`), because this PC has no dedicated graphics card. Checking
Ollama 0.34.4 on the owner's PC (ADR-014 step 0, the rules for adding AI tools; evidence in
`crates/runtime/tests/fixtures/ollama-0.34.4/`) showed:

- `ollama run <model>` reads the prompt from stdin and exits by itself, but prints plain text
  with terminal control codes, so it gives no structured output.
- The Ollama service the installer runs in the background listens on `127.0.0.1:11434` only.
  `POST /api/chat` streams one JSON object per line: thinking, answer, then `done` with the stop
  reason and token counts. It forwards cloud models to ollama.com under the signed-in account.
- Ollama keeps no conversations. Sent the earlier messages, the model answered from them.
- `POST /api/me` names the account and its plan (`"plan":"free"`), and answers `401` with a
  sign-in link when signed out. A cloud request while signed out also answers `401`.
- A model alone reads no files and runs nothing. Offered a tool, it asked for it
  (`tool_calls`) and ran nothing itself.
- No `OLLAMA_*` variables and no API key on this PC.

This fits ADR-014's intent (an official program, a subscription-style sign-in, no API key, and
structured output), but not two of ADR-007's rules (how Plenipo runs Claude Code and Codex):
there is no program to start per task, and the tool keeps no conversation. ADR-014 §7 says such a
tool needs its own decision record. This is that record.

## Decision

1. **Ollama is an AI tool reached through its service on this PC.** For a task, Plenipo sends
   `POST http://127.0.0.1:11434/api/chat` with `stream: true` and reads the streamed lines.
   - The address is fixed to `127.0.0.1:11434`; Plenipo ignores `OLLAMA_HOST` and never calls
     ollama.com itself.
   - The HTTP client is `reqwest`, already part of Plenipo's build through Tauri.
   - Plenipo does not start or stop the Ollama service. If it is not running, the AI tool shows
     "Not running — start Ollama", as for a tool that is not installed.
   - A task keeps what a supervised task has today: an ID, a time limit, cancel (closing the
     request, which stops the answer), the Ledger record, and restart handling ("interrupted").
2. **Cloud models only, under the owner's sign-in.**
   - Before every task, Plenipo calls `POST /api/me`. Signed in: the task runs, and the AI tool
     shows "Signed in (free plan)" or the paid plan's name. `401`: the task ends as "sign-in
     required", with the hint to run `ollama signin`.
   - Plenipo runs only cloud models (listed by `/api/tags` with `remote_host: https://ollama.com`,
     or named `…-cloud`), because this PC has no graphics card for local models. Local models can
     be allowed later by the owner.
   - Plenipo never uses or passes an Ollama API key (`OLLAMA_API_KEY`), and never sets one.
3. **Plenipo keeps the conversation.** A conversation's messages (the owner's objectives, the
   worker's answers, and any tool results) are stored with the session in the Ledger. Each task
   sends them, oldest first, within the model's context length; when they no longer fit, the
   oldest are left out and the worker is told so.
4. **Conversation only at first; tools later, through Guard.** An Ollama worker starts with no
   tools: it can answer, write, and review text, but not read files or run programs. Giving it
   Plenipo's tools (files, programs, git) is a follow-up on the Ollama branch: Plenipo offers them
   as Ollama tools, runs each requested call through the capability broker and Guard (ADR-013,
   how Plenipo lets workers use your computer safely) exactly as for the other AI tools, and
   sends the result back. The model never runs anything itself.
5. **Models and thinking.** The adapter declares the cloud models the owner chose and their
   thinking levels (ADR-014 §6), checked against `/api/tags` and `ollama show`. The owner's
   choices: `gpt-oss:120b-cloud` (checked), and deepseek-v4.1-flash, glm-5.3, glm-5.3-flash,
   minimax-m3, deepseek-v4-pro, and nemotron-3-ultra (exact names checked when the adapter is
   built). Thinking is sent as `think` (for gpt-oss: low, medium, high).
6. **Usage limits and errors.** `401` is "sign-in required". A usage limit (expected as `429`)
   is "usage limited", with its reset time when Ollama gives one; the free plan's exact message is
   recorded the first time it is seen. Anything else is "AI tool unavailable" with Ollama's error
   text.

## How it was built (2026-09-27)

Two details changed while building, within the decision above:

- **A helper run per task (§1).** Instead of calling the service from inside Plenipo, each task
  and sign-in check runs Plenipo itself in a helper mode (`plenipo-desktop --plenipo-ollama
chat|auth`), supervised like any AI tool's program. That keeps the task's ID, time limit,
  cancel (stopping the helper closes the request), and restart handling with no second way of
  running tasks. The helper is a small built-in HTTP client for `127.0.0.1` only, not `reqwest`;
  it prints one line per event (session, thinking, text, answer, done, error), and never the
  account's email or name (only the plan).
- **Where the conversation is kept (§3).** The helper keeps each conversation's messages in the
  session's own folder (`.plenipo-ollama-<id>.json`), beside the files of that worker's session.
  The Ledger still records every objective and answer, as for the other AI tools. When the
  history is longer than about 100,000 tokens, the oldest exchanges are left out and the model is
  told so. A task that fails leaves the history unchanged.
- **Models (§5).** The owner's eight cloud models are listed, with the thinking levels
  `ollama show` gives for each (evidence in `models/` of the fixtures): gpt-oss 120B (low,
  medium, high); Kimi K3, DeepSeek V4 Pro, DeepSeek V4.1 Flash, GLM-5.3, and GLM-5.3 Flash (low,
  high, max); MiniMax M3 and Nemotron 3 Ultra (no thinking setting: MiniMax lists no levels,
  Nemotron only on or off, on by default). Other cloud models can be named for a position as
  `ollama list` shows them.

## Consequences

- The owner gets frontier open models (DeepSeek, GLM, MiniMax, Nemotron, gpt-oss) through one
  free sign-in, without a graphics card or API keys.
- The runtime gains a second kind of AI tool, a service on this PC, beside programs started per
  task. Tasks, cancelling, and the Ledger behave the same to the rest of Plenipo.
- Plenipo stores conversation history for Ollama workers. It is the owner's data, in the Ledger,
  like every other task.
- Prompts to cloud models are processed on Ollama's servers, as prompts to Claude, Codex, and
  Kimi are processed on their companies' servers.
- Ollama workers cannot touch files until the tools follow-up, which is safer, but less useful for
  coding at first.
- The free plan's limits apply; a usage limit holds work back, as for the other AI tools
  (ADR-011, how Plenipo picks each worker's AI model).
- `ollama launch` can start other coding tools (Claude Code, Codex, Kimi Code, Hermes Agent, Muse
  Code, DeepSeek Harness, and more) on Ollama's models. That is out of scope here and would need
  its own checks and decision record.

## Alternatives considered

- **`ollama run` per task.** A supervised program per task, but plain text with terminal control
  codes, no token counts, and no conversation history.
- **Calling ollama.com directly with an API key.** Pay-per-use or key-based; it waits for the
  API use feature and its own decision record.
- **Local models.** No graphics card on this PC; they can be allowed later.
- **`ollama launch <tool>`.** Would run another coding tool on Ollama's models; it changes how
  that tool signs in and bills, so it needs its own checks.
