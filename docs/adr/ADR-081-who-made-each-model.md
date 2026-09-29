# ADR-081: Who made each model — the maker on every model, cross-company review by maker, and the model list two ways

- **Status:** Proposed (2026-09-29). The owner's answers to the choices in the
  [Phase 16 checklist](../phases/phase-16-checklist.md#choices-for-you) decide §5, §6, and §7.
- **Date:** 2026-09-29
- **Phase:** 16, Wave 1 (built beside Phase 20 by
  [ADR-080 (building Phase 16's first wave alongside Phase 20)](ADR-080-phase-16-wave-1-alongside-phase-20.md))
- **Carries out:** [ADR-036 (every AI model worth having)](ADR-036-every-ai-model.md) §3 and the
  Wave 1 part of §5.
- **Amends:** [ADR-011 (how Plenipo picks each worker's AI model)](ADR-011-model-policy-routing.md)
  §6 and §12 — cross-company review compares who made the models, not the AI tools' companies;
  [ADR-014 (adding AI tools)](ADR-014-adding-ai-tools.md) §6 — each model an AI tool lists also
  says who made it, and its Consequences' open question ("how the Router counts" a tool that runs
  other companies' models) is settled here.

> **On screen** (ADR-010, plain words and rank names): **Who made it** (the model's maker), **AI
> tool** (the app that runs it), **Group by: Who made it / AI tool**, **made by DeepSeek**, **not
> known** (who made a model), and **now** (what a name like "Opus" points to now: "Opus — now Opus
> 5.5"). Never "maker", "provider", "vendor", "runtime", "alias", or "snapshot".

## In short

Every model Plenipo knows says **who made it** — Anthropic, OpenAI, xAI, Moonshot AI, DeepSeek,
Z.ai, MiniMax, NVIDIA — apart from the **AI tool** that runs it. Today Plenipo only knows the AI
tool, so every Ollama model counts as "Ollama". That makes cross-company review wrong: OpenAI's
gpt-oss on Ollama can review Codex's work as if it came from another company, and a DeepSeek model
on Ollama can't review a GLM model on Ollama although two different companies made them. After this
change, cross-company review compares who made the models. Your list of models can be grouped by
who made them or by the AI tool that runs them, and Plenipo remembers your choice. Claude's names
("Opus") show the exact version they point to now, and the exact versions can be chosen too.
Accepting this record means building it as written below, with your answers to three choices.

## Context

ADR-011 §6 and §12: when a worker reviews work, a role can **prefer** or **require** a different AI
company than the one that did the work. The engine (`crates/router/src/engine.rs`) takes both
companies from the AI tools: the reviewer's is its AI tool's company (`info.provider`), and the work
under review is described only by the AI tool that did it (Liaison passes runtime IDs,
`crates/liaison/src/service.rs` `reviewed_work`). ADR-014 warned this would break for AI tools that
run other companies' models; Ollama (ADR-017) is the first. Its eight checked models come from six
companies:

| Ollama model (as Plenipo lists it)   | Who made it |
| ------------------------------------ | ----------- |
| gpt-oss 120B                         | OpenAI      |
| Nemotron 3 Ultra                     | NVIDIA      |
| Kimi K3                              | Moonshot AI |
| DeepSeek V4 Pro, DeepSeek V4.1 Flash | DeepSeek    |
| GLM-5.3, GLM-5.3 Flash               | Z.ai        |
| MiniMax M3                           | MiniMax     |

Claude Code, Codex, Grok, and Kimi run only their own company's models (Plenipo passes no variable
or flag that points them anywhere else; ADR-007 §4, ADR-014 §4).

## Decision

### 1. The maker on every model an AI tool lists

- **`KnownModel` gains `maker`:** the AI company that made the model, as an ID and a name
  (`deepseek`, "DeepSeek"). A maker that is also an AI tool's company uses that company's ID and
  name (`openai`, "OpenAI"; `moonshot`, "Moonshot AI"), so gpt-oss on Ollama and GPT-6-Sol on
  Codex are one company.
- **Each AI tool also says** who made its default model (Ollama: gpt-oss 120B, so OpenAI) and
  whether it runs only its own company's models (Claude Code, Codex, Grok, Kimi: yes; Ollama: no).
- **The adapters keep this knowledge** (ADR-003, ADR-014 §7): the Router, Liaison, and the screens
  stay free of company names; they read the maker from the AI tool's list.
- **The contract suite checks:** every listed model has a maker with an ID and a name; one name per
  ID across all AI tools; and a tool that runs only its own company's models lists only models its
  company made.

### 2. Who made a model that is not on the list

- **An AI tool that runs only its own company's models:** its company. A new Codex model is
  OpenAI's.
- **The AI tool's default ("Its default"):** the maker of its default model.
- **An AI tool that runs other companies' models (Ollama):** **not known**, unless the model is on
  its checked list. This happens for a model you typed, or one Ollama reports that Plenipo has not
  checked ("new — not checked yet"). What Plenipo does with it is **choice 3** (§7).

The maker is worked out each time from the AI tool's list, not saved in your settings, so a
Plenipo update that adds or corrects a maker applies at once. Your model list is unchanged; no new
Ledger layout.

### 3. Cross-company review counts who made the models

- **The work under review** is described by the AI tool **and the model** that did it: the model
  the task asked for, else the model its result reported, else the AI tool's default. Liaison
  passes both to the directory (`Directory::place`), and the Router works out each one's maker.
- **Prefer:** models whose maker differs from every reviewed maker go first. **Require:** a model
  whose maker made any of the reviewed work is skipped, with the reason.
- **The reason names makers:** "It comes from a different AI company than the work it reviews
  (DeepSeek)."
- **The plan's two tests hold:** two models with the same maker on different AI tools (Kimi K3 on
  Kimi, and Kimi K3 on Ollama) count as **one** company; two makers inside Ollama (DeepSeek and
  Z.ai) count as **two**.

### 4. What stays as it is

- **What a usage limit does** (ADR-011 §8, "wait"): work never moves to another AI tool's company
  because of a usage limit. That rule is about which subscription pays, so it keeps comparing the
  AI tools' companies. Moving Kimi's work to Kimi K3 on a paid Ollama plan when the Kimi
  subscription runs out is Wave 3's routes (ADR-036 §4), not this wave.
- **Pay-per-use API billing** stays off; nothing about paid keys changes (Wave 3).
- **Choosing a model** works as today; the menus add who made each model where an AI tool runs
  more than one company's models.

### 5. AI companies never to use — choice 2

"Never use these AI companies" (ADR-041) today compares the AI tool's company. **Recommended:** a
company on the list is skipped whether it **made** the model or its AI tool **runs** it, and the
list offers the makers too (DeepSeek, Z.ai, MiniMax, NVIDIA). So "never OpenAI" also skips
gpt-oss on Ollama, and "never Ollama" still skips every Ollama model. The other answer keeps
today's rule (the AI tool's company only) for this wave.

### 6. The model list, grouped two ways — choice 1

- **Recommended:** Settings → AI models → **Your models** (the list your roles choose from) gets
  **Group by: Who made it / AI tool**. Each group has a heading; each row shows both who made it and
  the AI tool that runs it; both groupings show exactly the same models.
- **Every place a model is listed** says who made it: each AI tool's **Models** tab on the AI tools
  page ("made by DeepSeek"), and the model menus for an AI tool that runs more than one company's
  models.
- **Remembered on this PC,** like the light or dark theme and the side panel's width (the app's
  own stored choices, `useStoredState`). It starts on **AI tool**, today's look, until you choose.
  No new desktop command is needed.

### 7. A model whose maker is not known — choice 3

**Recommended — play it safe:** it shows **Who made it: not known**. A reviewer that must come
from a different company never uses it, and work done by it is never counted as coming from a
different company than anything, so a "must be different" review of such work finds no reviewer
and says why. A "prefer different" review tries it last. The other answers: count it as a
company of its own (never the same as a known one), or let you say who made it when you add it to
your list.

### 8. Exact Claude versions beside the plain names

- **`KnownModel` gains `points_to`:** for a name that follows the newest model (Claude Code's `fable`,
  `opus`, `sonnet`, `haiku`), the exact model it points to as of the Claude Code version Plenipo
  checked (`checked_version()`). The screens show "Opus — now Opus 5.5 (`claude-…`)".
- **The exact versions are listed too,** as models of their own, so a role can stay on one version
  when a new one comes out. Each keeps the effort levels of its name.
- **Only what the owner's PC shows:** the version each name points to comes from Claude Code's own
  report at the start of a task (`system` / `init`, `model`), and each exact version is added only
  after a task with it succeeds on the owner's subscription. Nothing is taken from memory.

### 9. Older OpenAI models a ChatGPT sign-in allows

Codex's list gains each **older** model that a one-word task on the owner's ChatGPT sign-in
actually ran, with the effort levels Codex reports for it (its app server's `model/list` with
`includeHidden: true`, OpenAI's documented call). A model that is refused is not listed. They
come after today's models in the menus. Models Codex keeps out of its own picker that are not
older models (on Codex 0.145.0: GPT-Reserve and Codex Auto Review) stay out, as ADR-011 §16 lists
what the AI tool's own picker offers; they can still be typed. On the owner's PC, every older model tried
was refused, on Codex 0.145.0 and 0.159.0, so none is added.

### 10. Google's AI tool and more Ollama models

These follow their own rules and are not decided here. Gemini CLI failed ADR-014's bar on the
owner's PC: Google no longer serves it to personal plans ([the finding](../phases/ai-tools-gemini-finding.md)).
At the owner's direction, Google's replacement, Antigravity CLI, goes through the same bar with
step 0 on the owner's PC, and gets its own decision record (ADR-082) if it passes, or a finding if
it does not. More Ollama cloud models are added only if the owner's paid plan is active, each
checked on the owner's PC as ADR-017 did. Each new model says who made it (§1).

## Consequences

- Cross-company review becomes correct for Ollama's models, and stays exactly as it was for Claude
  Code, Codex, Grok, and Kimi (each runs only its company's models, so maker and company are the
  same).
- The owner can see, for every model, who made it and which AI tool runs it.
- Every AI tool's list now needs a maker for each model; the contract suite refuses a list without
  one. A future AI tool that runs other companies' models (Cursor, Copilot, OpenRouter) only has
  to fill in the maker; no decision record is needed for how it is counted (ADR-036 §3.4).
- A model Plenipo does not know cannot make a "must be different" review pass (§7,
  recommended). The fix is a Plenipo update that lists it, or the owner choosing a known model.
- Claude's exact versions and the older OpenAI models age: a later Claude Code or Codex version
  may stop accepting one. As today, a name the AI tool refuses fails that task with the tool's own
  message, and the next check updates the list with its version.

## Alternatives considered

- **Keep counting the AI tool's company** (today). Rejected by ADR-036: it makes cross-company
  review meaningless for any AI tool that runs other companies' models.
- **Save the maker in the owner's model list.** Rejected: an old saved maker would outlive a
  corrected one; working it out from the AI tool's list each time keeps one source.
- **Guess the maker from a model's name** (for example "deepseek" in `deepseek-v4-pro:cloud`).
  Rejected: names are not a promise, and a wrong guess would pass a review it should not.
- **Move the usage-limit rule to makers now.** Rejected for this wave: it would move work between
  subscriptions, which is Wave 3's routes, built with spending caps.
- **Keep the grouping choice in the Ledger** (a new setting and desktop command). Not needed: it
  only changes how a list looks on this PC, like the theme.
