# ADR-084: Cursor's agent waits — no check a program can run for paid extra use

- **Status:** Accepted (2026-09-30). The owner's answer to choice 1 of the
  [Wave 2 checklist](../phases/phase-16-wave-2-checklist.md#the-owners-choices-2026-09-30):
  "as recommended. Grok is cursor."
- **Date:** 2026-09-30
- **Phase:** 16, Wave 2 ("one AI tool, one decision record each")
- **Follows:** [ADR-014 (adding AI tools)](ADR-014-adding-ai-tools.md) §4 — a tool that fails the
  bar gets a written finding, not a workaround.

> **On screen:** nothing. Cursor's agent is not added, so no screen names it.

## In short

Cursor's command-line agent fails one rule of the bar for new AI tools: nothing a program can run
says whether Cursor may charge for paid "on-demand" use once the plan's included usage runs out.
Plenipo must be sure of that before every task, so Cursor's agent is **not added now**; a written
finding says why and what would change the answer. Accepting this record means Plenipo waits for
Cursor, and keeps reaching xAI's models through Grok.

## Context

- The plan's Wave 2 lists "Cursor's agent (its own models plus Anthropic's, OpenAI's, Google's,
  xAI's, Moonshot's)". Anything that runs on Cursor's own computers is out of scope.
- **The build machine** (`cursor-agent` 2026.09.28-64d2043, signed out): it reads the task from
  standard input, reports JSON lines, continues a chat by its ID, and has a sign-in check. Its
  on-demand setting is read only by its own `/usage` screen, from Cursor's service; no command,
  no ACP message, and nothing in a task's output reports it
  ([the finding](../phases/ai-tools-cursor-finding.md)).
- **The owner's PC** (2026-09-30): the `agent` command is xAI's Grok program ("grok 1.0.44"); the
  owner's Cursor account is on the **Free** plan, linked to SuperGrok, with **On-Demand Spending:
  Disabled**. The owner: "Grok is cursor."
- GitHub Copilot failed the same way on its first try, and passes now because its own link reports
  the setting ([ADR-083 (GitHub Copilot as an AI tool)](ADR-083-github-copilot-as-an-ai-tool.md)).

## Decision

1. **Cursor's agent is not added.** The finding
   [`docs/phases/ai-tools-cursor-finding.md`](../phases/ai-tools-cursor-finding.md) is merged in its
   place.
2. **It is tried again** when Cursor adds a way for a program to see on-demand use (in
   `agent status` or `agent about`), or a way to stop at the included usage — with step 0 on the
   owner's PC, as for every AI tool.
3. **xAI's models stay on Grok**, Plenipo's AI tool for them since
   [ADR-015 (running AI tools over ACP)](ADR-015-acp-ai-tools.md). The owner's Grok program is the
   one the `agent` command runs on the owner's PC. Plenipo finds Grok by its own name (`grok`), so
   the `agent` name changes nothing.

## Consequences

- Cursor's own models (and its route to other companies' models) are not reachable from Plenipo
  for now. Anthropic's, OpenAI's, Google's, xAI's, and Moonshot AI's models stay reachable through
  their own AI tools.
- Nothing changes on screen, in the Router, or in any record.

## Alternatives considered

- **Trust the owner's setting** (On-Demand Spending: Disabled). Plenipo could not see if it were
  turned on later; the first Copilot finding ruled this out for the same reason.
- **Read the setting from Cursor's own screen, or from its service with the stored sign-in.**
  Driving a tool's screen and unofficial clients are ruled out by ADR-014 §7.
- **Run it with a key** (`CURSOR_API_KEY`). Keys are never used (ADR-007 §4).

## As built

Merged on 2026-09-30 (v1.15.0) as decided: the finding
([`ai-tools-cursor-finding.md`](../phases/ai-tools-cursor-finding.md)), with no adapter, persona, or
registration. The setup guide says Cursor's agent is not an AI tool in Plenipo yet, and why.
