# ADR-014: Adding AI tools ahead of Phase 15

- **Status:** Accepted (by the owner, 2026-09-26). Bar item 1 is widened by
  [ADR-015](ADR-015-acp-ai-tools.md) (running AI tools over ACP): the prompt may also go in on
  stdin over ACP.
- **Date:** 2026-09-26
- **Phase:** 15 (adapter parts pulled forward, after v0.7.0)

## Context

The owner wants Plenipo to run models from every frontier AI lab, not only Anthropic's (through
Claude Code) and OpenAI's (through Codex). The next three are Google's Gemini, GitHub Copilot,
and xAI's Grok. Branches for them already exist: `claude/ai-tools-gemini`,
`claude/ai-tools-copilot`, and `claude/ai-tools-grok`.

The rollout plan puts new AI tools in Phase 15 ("Additional Providers and Department
Expansion"), after a stable production architecture. Phase 15 asks for a documented provider
adapter SDK/contract, a model capability discovery contract, a provider adapter contract suite,
and an additional adapter "when justified". It warns: "Do not add providers merely to increase
a logo count." A provider is added only when it has a supported programmatic runtime,
acceptable authentication, useful capability, clear role fit, and stable enough execution
semantics.

The adapter contract itself has been stable since Phase 3. ADR-007 (how Plenipo runs Claude Code
and Codex) set the rules every adapter follows:

- the official non-interactive CLI, one supervised process per turn;
- the prompt on stdin, never in the arguments;
- subscription sign-ins only, checked before every turn with the CLI's own status command;
- no credentials passed to the CLI;
- least privilege until Plenipo Guard grants permissions (Phase 7).

Phase 7 (Guard) is being built in parallel on `claude/phase-7`. It changes the same adapter
files (`crates/runtime/src/agent/*`): each turn can get Plenipo's own tool server, and adapters
can add variables per turn.

## Decision

1. **Pull forward only the adapter parts of Phase 15.** They are the documented adapter contract
   ([`docs/development/adding-an-ai-tool.md`](../development/adding-an-ai-tool.md)), the model
   capability discovery contract (§6), and the contract suite
   (`crates/runtime/tests/contract.rs`). The rest of Phase 15 stays where it is: new department
   templates, role packs, and import/export of organization settings.
2. **The bar.** An AI tool is added only if it passes all five, checked on the real CLI:
   1. **An official CLI with a non-interactive mode.** The CLI comes from the AI company (or,
      for Copilot, GitHub). It runs one task and exits by itself, reads the prompt from stdin,
      and never stops to ask a question.
   2. **Structured or streaming output.** One JSON event per line (or an equivalent the parser
      can read line by line) that reports the session ID, the final answer, and errors.
   3. **Subscription sign-in only.** The owner signs in once with the tool's own login command
      and the subscription account. Pay-per-use API billing stays off. Plenipo never asks for,
      stores, or passes API keys or passwords.
   4. **A sign-in status check that can tell a subscription from an API key.** This is a
      command Plenipo runs before every turn, as with `claude auth status` and
      `codex login status`.
   5. **Stable execution.** A version flag, a session that can be resumed by ID, documented
      exit behavior, and the same results when the owner repeats the check.
3. **One branch and one pull request per AI tool.** The branches are `claude/ai-tools-gemini`,
   `claude/ai-tools-copilot`, and `claude/ai-tools-grok`. Each merges `main` after this
   groundwork lands, and again after Phase 7 lands. Before any adapter code is written, the
   branch checks the bar against the real CLI (the guide's step 0). The owner runs the listed
   commands and the outputs go into the tool's checklist in `docs/phases/`. Those outputs are
   also what the parser and the fake CLI must reproduce.
4. **A tool that fails the bar gets a written finding, not a workaround.** The finding goes in
   `docs/phases/ai-tools-<tool>-finding.md` and says:
   - which bar item failed, with the evidence;
   - what would change the answer, for example a CLI release that adds a status command.

   The branch merges the finding instead of an adapter, and the tool can be tried again when
   that changes.

5. **What each adapter provides.** Each adapter implements every item of the `RuntimeAdapter`
   trait (`crates/runtime/src/agent/adapter.rs`), as the guide explains with Claude Code and
   Codex as worked examples:
   - its identity and AI company;
   - finding the executable and reading its version;
   - the sign-in check and billing classification;
   - the variables it passes (never the API-key ones);
   - turn arguments for model, effort, and resume;
   - the output parser and normalized result, including usage limits and sign-in errors;
   - what it can do (`capabilities`, §6).

   Each tool branch also adds:
   - a persona in `plenipo-fake-agent`;
   - one line in `builtin_adapters()`;
   - a section in the setup guide.

   The adapter starts with the least the CLI allows: no writes and no network until Guard grants
   permissions. If the CLI cannot be limited that way, the branch stops and asks the owner.
   Every adapter must pass the contract suite. The suite runs for every adapter in
   `builtin_adapters()`, so registering the tool is what puts it under test.

6. **Model capability discovery contract.** `capabilities()` declares:
   - the effort levels the CLI accepts;
   - the models the CLI itself offers (the names its model option takes, how it labels them,
     and each model's own effort levels);
   - and `checked_version()` records the CLI version they were checked against.

   Plenipo offers these models in its menus and never adds them to the owner's list
   (ADR-011, how Plenipo picks each worker's AI model). A newer CLI may offer models that are
   not listed yet. The owner can still type a model's name, and the next check updates the list
   and the version together.

7. **Out of scope.** These stay out:
   - pay-per-use API billing, including "bring your own key" modes;
   - unofficial or third-party clients;
   - scraping web sessions or reusing browser sign-ins;
   - driving a CLI's interactive screen;
   - tool-specific logic outside the adapter.

   Outside its adapter module, a tool branch touches only the registration line, its fake-CLI
   persona, the docs, and screen text that lists the AI tools by name. Core, Liaison, Router,
   Workforce, Guard, and the Ledger stay tool-neutral, as ADR-003 requires. If a tool seems to
   need more, that is a new decision record.

## Consequences

- Each tool branch works from one guide and gets the contract suite and the fake-CLI test
  helpers without editing them. The helpers install every persona the fake CLI lists, and the
  desktop's IPC tests expect exactly the registered AI tools.
- The owner checks each tool on the real CLI twice: before code (step 0) and at acceptance. CI
  has no accounts and only runs the fakes.
- Phase 7 changes the adapter contract while the tool branches are open. Each tool branch merges
  `main` after Phase 7 lands. It then either wires in Plenipo's tool server the way Phase 7
  documents, or keeps the conversation-only posture and says so.
- Some screen text still says "Claude Code and Codex". The first tool branch updates it to cover
  every AI tool; the guide lists where.
- Some AI tools offer models made by other AI companies. GitHub Copilot is one. For those tools,
  "a different AI company" in cross-company review is no longer a property of the tool alone.
  Such a branch settles how the Router counts it in its own decision record before merging.
- Model lists go out of date as CLIs update. `checked_version()` shows how old each list is.

## Alternatives considered

- **Wait for Phase 15 as planned.** The owner wants these models now, and the adapter contract
  has not changed since Phase 3 apart from Phase 7's additions.
- **All three tools on one branch.** That makes a larger review, one tool's blocker would hold
  back the others, and it would collide more with Phase 7.
- **Pay-per-use APIs** (for example, a Gemini or xAI API key). This would break the
  subscription-only rule: ADR-003 (roles never tied to an AI company, and no paid API
  fallback unless the owner turns it on) and ADR-007 §4 (credentials and billing).
- **Third-party multi-model CLIs or community wrappers.** They are unofficial, and their billing
  and credential handling are outside the AI company's terms and Plenipo's control.
- **Adapters loaded at run time (plugins).** This adds an attack surface and a loading mechanism
  Plenipo does not need yet. Adapters stay compiled in and reviewed.
