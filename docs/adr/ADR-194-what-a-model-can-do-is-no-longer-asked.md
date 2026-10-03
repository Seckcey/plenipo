# ADR-194: What a model can do, and its context size, are no longer asked

- **Status:** Accepted (the owner, 2026-10-03: "What's the point of asking if models can view
  images, create images and/or use computer? We really need to simplify Plenipo", and "Auto
  populate context size in Settings → AI Models or remove it"; items 2.3 and 2.4 of
  [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md), accepted the same day).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 2 (items 2.3 and 2.4)
- **Amends:** [ADR-011 (model policy routing)](ADR-011-model-policy-routing.md): a model's "what
  it can do" and context size, and a role's required capabilities and minimum context.
  [ADR-042 (specialties)](ADR-042-specialties.md): a specialty no longer suggests either.

> **On screen** (ADR-010, plain words and rank names): Settings → AI models no longer has "It can
> also", "Can also", "Context", "The model must be able to", or "Context size, at least". A
> worker whose model is known to see images is told "Your AI model can see images."

## In short

Plenipo asked you to tick what each model can do (sees images, makes images, uses a computer)
and to type its context size. Nothing filled these in for you. A model left unticked was treated
as unable, so:

- every worker was told its model "is not marked as able to see images", even Claude and GPT;
- the **Designer got no model at all** out of the box, because its role asked for a model that
  sees and makes images;
- a role with a minimum context size skipped every model whose size you hadn't typed.

**Accepting this record means:** these questions are gone. They never rule a model out.

## Decision

1. **The router ignores them.** A model's saved "what it can do" and context size, and a role's
   saved requirements and minimum, are kept in saved settings so old settings still load (saved
   settings refuse unknown fields), but the router no longer skips a model for them.
2. **The screens no longer ask.** Settings → AI models (the model list, its form, and each role's
   choices), the specialty dialog, and the agent's AI model tab drop them. Saving a model or a
   role keeps what was saved before, untouched.
3. **"Makes images" and "Uses a computer" are gone.** Plenipo turns image-making off for workers
   (Grok's `image_gen`, Codex's `view_image`), and using a computer is decided by Guard's computer
   permissions, not by a box.
4. **"Sees images" is worked out, not asked.** It comes from who made the model (ADR-081, who made
   each model): every current model from Anthropic and Google sees images, and OpenAI's do apart
   from its open-weight `gpt-oss` models. A worker on one of those is told "Your AI model can see
   images." When it isn't known, the worker is told nothing.
5. **No one-time fix is needed.** Because the router ignores saved requirements, a Designer saved
   by an older version gets a model too. New installs start the Designer with no requirements,
   and the built-in specialties suggest neither.

## Consequences

- The Designer works out of the box, on a fresh install and after an upgrade.
- Fewer fields in Settings → AI models.
- A role can no longer insist on a model that sees images. When the work needs one, its lead can
  choose that model for the role by name.

## Alternatives considered

- **Fill the context size in by itself.** Rejected: no AI tool's model list includes it, and
  finding it would mean testing each model on your PC again and again (ADR-081 §8), for a rule no
  built-in role uses.
- **Keep "Sees images" as a box, ticked for you.** Rejected: one more question, and it would still
  block the Designer whenever Plenipo couldn't tell.
