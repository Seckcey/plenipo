# AI tools groundwork — Implementation Checklist

**Status:** in review; waiting for the owner to accept ADR-014 (see the
[acceptance report](ai-tools-groundwork-acceptance-report.md)).

Source: `ROLLOUT_PLAN.md` Phase 15 — Additional Providers and Department Expansion. Only its
adapter parts are pulled forward:

- "documented provider adapter SDK/contract";
- "model capability discovery contract";
- the "provider adapter contract suite".

The owner asked for this on 2026-09-26 on branch `claude/ai-tools`, from `main` at v0.7.0. The
reason is in ADR-014 (adding AI tools ahead of Phase 15). This checklist keeps the plan's words
where it quotes the plan. The app uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md).

**Goal:** lay the groundwork so each new AI tool can be added on its own branch. This branch adds
no AI tool.

## Order of work

1. **Groundwork**: this branch. Merge to `main` first: the tool branches wait on it.
2. **Gemini** (`claude/ai-tools-gemini`), **Copilot** (`claude/ai-tools-copilot`), and **Grok**
   (`claude/ai-tools-grok`), each on its own branch and its own pull request.
   - Each merges `main` after the groundwork lands.
   - Each starts with step 0 of the [adapter guide](../development/adding-an-ai-tool.md) (the
     check on the real CLI), then writes either an adapter or a finding.
   - They can be worked on side by side, but they merge one at a time. Each one adds a line to
     `builtin_adapters()`, a persona, and a setup-guide row, so the next branch merges `main`
     again before it merges.
3. **Phase 7** (Guard) continues in parallel on `claude/phase-7`. Tool branches merge `main` again
   once Phase 7 lands, then wire in Plenipo's tool server or stay conversation-only.

## Design decisions (details in ADR-014, adding AI tools ahead of Phase 15)

- **The bar**, checked on the real CLI before code:
  - an official CLI with a non-interactive mode;
  - structured or streaming output;
  - subscription sign-in only;
  - a sign-in status check that tells a subscription from an API key;
  - stable execution.

  A tool that fails gets a written finding, not a workaround.

- **Adapters stay the only tool-specific code.** A tool branch touches only its adapter module,
  one line in `builtin_adapters()`, a fake-CLI persona, the docs, and screen text that lists the
  AI tools by name.
- **The contract suite runs for every registered adapter**, so a tool branch gets it by
  registering its adapter.
- **Model capability discovery** stays a hand-checked list in each adapter, like Phase 6's model
  menus. The new `checked_version()` records the CLI version the list was checked against.

## Deliverables

- [x] Decision record: ADR-014 (adding AI tools ahead of Phase 15), **Proposed**, in the
      [ADR index](../adr/README.md)
- [x] Adapter guide, the plan's "SDK/contract":
      [`docs/development/adding-an-ai-tool.md`](../development/adding-an-ai-tool.md). It walks
      through the `RuntimeAdapter` trait with Claude Code and Codex, and ends with a checklist
      for tool branches and a template for findings.
- [x] Model capability discovery contract:
  - `RuntimeAdapter::checked_version()`: Claude Code 2.1.283, Codex 0.157.1;
  - the rules for `effort_levels` and `known_models` (guide §7).
- [x] Contract suite: [`crates/runtime/tests/contract.rs`](../../crates/runtime/tests/contract.rs),
      which runs for every adapter in `builtin_adapters()`
- [x] Test harness ready for more AI tools:
  - [x] `plenipo-fake-agent`: personas are one table (`PERSONAS`), and `--personas` lists them
  - [x] Rust harnesses (runtime, Liaison, Workforce) install every persona (`personas()`)
  - [x] End-to-end tests install every persona through one helper, `installFakeTools(home)` in
        `tests/e2e/lib/app.mjs`
  - [x] The desktop IPC tests expect exactly the adapters in `builtin_adapters()`. The
        unknown-tool example is now `no-such-tool`, not `gemini`.
- [x] `TurnRequest` and `ProviderSession` have defaults, so tests build requests with
      `..TurnRequest::default()` and keep compiling when Phase 7 adds `tools`
- [x] Architecture overview §6 links the guide and the suite

## Contract suite (what every adapter must pass)

- [x] ID, label, and AI company are filled in; IDs, labels, and executables are unique; one name
      per company ID; IDs are lowercase letters, digits, and dashes
- [x] Known model names pass `validate_model` and are listed once
- [x] Each model's effort levels are within the tool's, lowest first
- [x] `checked_version()` is a CLI version
- [x] Turn arguments never carry a credential, and they carry the chosen model, the effort, and
      the session ID (resume, and new with an ID Plenipo chose). The check covers every request
      shape.
- [x] No API-key, token, or cloud-billing variables are passed through
- [x] Malformed lines are ignored and counted, never stop the turn, and never crash the parser
- [x] `finish()` gives exactly one result for every way a process ends (success, failure, could
      not start, cancelled, timed out, interrupted), and it is never "completed" without a
      reported completion
- [x] On the fake CLI:
  - the sign-in check tells a subscription (ready) from an API key (never ready) and from signed
    out;
  - the prompt reaches the CLI on stdin, never in its arguments;
  - a usage limit and an expired sign-in are normalized.

## Verification (before every push, from CLAUDE.md)

- [x] `pnpm check`
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [x] `cargo test --workspace --locked`
- [x] `pnpm bindings`, with no diff in `packages/types/src/generated`
- [x] `pnpm e2e` against the release build (the fake CLI and the e2e helpers changed)

## Owner decisions

- [ ] Accept ADR-014 (adding AI tools ahead of Phase 15). Accepting it means:
  - the bar and the one-branch-per-tool process apply to Gemini, Copilot, and Grok;
  - a tool that fails the bar gets a finding instead of an adapter.

## Out of scope

- Any new AI tool (each has its own branch).
- The rest of Phase 15: department templates, role packs, and organization import/export.
- Screen text that still says "Claude Code and Codex": the first tool branch updates it (the
  guide lists where).
- Plenipo's tool server for new AI tools (Phase 7).
