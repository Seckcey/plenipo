# ADR-190: Phase 25 starts — fixes and a simpler Plenipo before launch, in four waves

- **Status:** Accepted (by the owner, 2026-10-03). The owner chose the work in two lists, then
  answered the six questions: start as soon as possible, beside the other phases, "as long as you
  don't step on each other's work"; questions 2, 3, 4, and 6 "as recommended"; and on question 5,
  the resets AI companies give out now and then (see
  [the owner's answers](#the-owners-answers-2026-10-03)).
- **Date:** 2026-10-03
- **Phase:** 25 (new)
- **Amends:** [ADR-132 (the final push: Phase 14, then Mac and Linux, then Community)](ADR-132-the-final-push.md):
  it adds Phase 25 to the order of work. The rules it changes inside earlier records are listed
  under [What it changes](#what-it-changes).
- **Number:** on 2026-10-03, `main` and every branch on GitHub stopped at ADR-172. Phase 24 had
  run past its block (160 to 169) into the 170s. So that no two sessions pick the same number,
  **Phase 25 uses ADR-190 to ADR-199**, starting with this one.

> **On screen** (ADR-010, plain words and rank names): every new word follows
> `docs/design/vocabulary.md`. New pairs go there as they are built: "Stop all work", "Allow
> again", "Side chat", "Live conversation", "Subscription connected", "API key connected".

## In short

On 2026-10-03 the owner tried Plenipo as a new user would, and sent two lists.

- **The first list:** ideas for before launch. The owner kept six:
  - managing token use
  - templates
  - a suggestion when a plan runs out
  - prompt caching
  - simpler Connections cards
  - catching made-up answers

  The rest were dropped.

- **The second list:** 17 bugs and changes, from a freeze when making an organization to a missing
  Stop all button.

Plenipo's builder checked every item against the code (v1.20.0), found the cause of each bug, and
wrote one plan: [the Phase 25 checklist](../phases/phase-25-checklist.md).

**Accepting this record means** Phase 25 is added to the plan, uses ADR-190 to ADR-199, is built in
**four waves**, and changes the earlier rules listed below.

| Wave | What                                                                    | Rough size      |
| ---- | ----------------------------------------------------------------------- | --------------- |
| 1    | Fix what's broken                                                       | 2 to 3 sessions |
| 2    | Make Plenipo simple: fewer questions, closed cards, templates, the tour | 7 to 9 sessions |
| 3    | See and steer the work: live output, Stop, Stop all, side chats         | 5 to 7 sessions |
| 4    | Make AI plans last, and catch made-up answers                           | 6 to 8 sessions |

## Context

The final push (ADR-132) has three phases: 14 (delivered), 23 (in progress, Wave 1), and 24 (being
built beside it). No session may start other work unless the owner schedules it. The owner has now
asked for this work and called it "before launch".

What the check found (details and file paths are in the checklist):

- **The freeze** when making an organization comes from opening its window inside a sync command.
  On Windows that locks the window thread for the whole app.
- **The AI tools page reads only the first organization.** Usage, plan checks, and paid keys all
  live with the first organization. A second organization's window shows the wrong usage numbers,
  and "Key not in use" for a key that works.
- **Subscriptions and their keys are separate AI tools** with separate cards and lights. Since
  v1.17.0 the key tools carry the company's name ("xAI", "Anthropic"), so choosing "xAI" for a
  manager picks the paid key, not the Grok subscription.
- **Picking a model takes 3 screens and 4 forms.** Several settings do nothing until filled in
  (context size, image boxes, cost class). "Never use" lists add up across four levels, and an
  unknown maker (Copilot) is refused whenever any list has any company.
- **"Set up a Development project" always hires a new team**, and a supervisor can reach only its
  own team.
- **Watch shows only the clicked worker's own file writes**, so a manager's or supervisor's Watch
  is always empty. Live text exists, but only on the Workers page.
- **Stop all stops control, not AI work.** There is no Stop on most worker popups. There is no way
  to ask a busy agent a question.
- **Plenipo never asks Anthropic to cache** (ADR-085 §3.5), and nothing paces plan use over time.
- **Plenipo keeps the real facts of each task** (files, tests, pull requests) but shows them only
  to the owner. Supervisors never see them.

## Decision

1. **Phase 25** is added to `ROLLOUT_PLAN.md`, after Phase 24, with
   [its checklist](../phases/phase-25-checklist.md). It uses ADR-190 to ADR-199.
2. **Four waves, in order:** fix what's broken, make it simple, see and steer the work, then plans
   and made-up answers. Inside Wave 2, the setup tour comes last, because it walks over the simpler
   screens.
3. **It starts now** (2026-10-03), **beside Phases 23 and 24**, not instead of them. No phase
   pauses. Each wave of Phase 25 is built on its own branch from the latest `main`, merges `main`
   often, and stays out of files another open branch is changing; where it must touch a shared file
   (the command list in `lib.rs`, `App.tsx`, `views.ts`, the vocabulary list), it keeps the change
   small so the other sessions' merges stay easy.
4. **Dropped from the first list** at the owner's direction (2026-10-03): the workflow canvas, the
   command-line version, installing AI tools for the owner, making every number clickable (only
   "What's stuck" and "Objectives going" stay, in item 1.7), and the website copy. ADR-059 (Plenipo
   keeps the AI tools up to date) is unchanged.

### What it changes

Each change is written into its own record, or as an amendment, when its item is built. These lines
are what the owner's lists already decided:

| Record                                                                                  | Change                                                                                                                                                             | Item     |
| --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------- |
| [ADR-011 (the router's model rules)](ADR-011-model-policy-routing.md)                   | "Makes images", "Uses a computer", and context size leave the screen; saved values stay readable and are ignored; "Sees images" comes from the AI tools' own lists | 2.3, 2.4 |
| [ADR-016 (the Development department)](ADR-016-development-department.md)               | Setting up a project reuses the department's workers and hires only what's missing                                                                                 | 2.7      |
| [ADR-030 (one design system)](ADR-030-design-system.md)                                 | Driver.js (MIT) is allowed, for the setup tour only                                                                                                                | 2.9      |
| [ADR-041 (model and effort in layers)](ADR-041-model-effort-learning-layers.md)         | One "who uses what" table; "never use" lists set at the organization level                                                                                         | 2.6      |
| [ADR-054 (move or lend an agent)](ADR-054-move-or-lend.md)                              | A supervisor may hand work to department workers outside its own team                                                                                              | 2.7      |
| [ADR-055 (Watch)](ADR-055-watch-a-worker-write-code.md)                                 | Watch shows the whole team's changes, and changes made by commands                                                                                                 | 1.8, 3.2 |
| [ADR-081 (who made each model)](ADR-081-who-made-each-model.md)                         | A model picked by name is the owner's informed choice: an unknown maker warns instead of refusing                                                                  | 1.6      |
| [ADR-085 (paid AI keys with spending caps)](ADR-085-paid-ai-keys-with-spending-caps.md) | §3.5: Anthropic prompt caching is allowed. §6: a chosen model runs on the subscription first, then on the same company's key                                       | 4.1, 4.4 |
| [ADR-094 (more than one organization)](ADR-094-more-than-one-organization.md)           | Usage adds up across organizations. Paid keys follow question 2                                                                                                    | 1.2, 1.4 |

**Unchanged:**

- Plan rule §3.2, "no silent provider switching", holds. Every step down and every move to a key
  is shown and recorded.
- ADR-060 (usage only from what the AI tools report) holds. Plenipo paces by the percentages the
  AI tools report, and labels its own estimates as estimates.
- Workers still never control each other. Only supervisors and up may stop or send back work, and
  that is recorded (item 4.8).
- Plenipo never buys usage for the owner, and never uses a reset for them (item 4.2).

## Questions for the owner

1. **When does Phase 25 start?**
   - **Recommended:** Wave 1 now, beside Phase 23, because these bugs hit anyone using Plenipo
     today. Then Waves 2 to 4 before launch, ahead of Phase 23's Waves 2 to 4 and the parts of
     Phase 24 not yet built.
   - Other choices: after Phase 23, or after Phase 24.
2. **Paid keys: one set for the whole PC, or each organization its own?**
   - **Recommended:** one set for the whole PC. Usage is already the PC's (ADR-094 §4). Spending
     caps stay per organization.
3. **When a manager needs a job no one holds: ask first, or hire on its own?**
   - **Recommended:** ask first, with a switch in Settings → Switches to let it hire on its own.
4. **Side chats: answer only, with no tools and no hand-offs?**
   - **Recommended:** yes. A side chat can't change files or start work. It only answers.
5. **"Subscription reset": does it mean buying extra usage on the AI company's own site?**
   - The plan assumes yes. Plenipo shows the button and opens the company's page, and never buys
     anything itself.
6. **Stepping down (lower effort, smaller model, then the key): on by default, or off until you
   turn it on?**
   - **Recommended:** on, with every step shown and recorded, and a switch to turn it off.

## The owner's answers (2026-10-03)

1. **Start as soon as possible**, beside the other phases, "as long as you don't step on each other's
   work". Decision 3 above.
2. **One set of paid keys for the whole PC.** Spending caps stay per organization (item 1.4).
3. **Ask first** when a manager needs a job no one holds, with a switch to let it hire on its own
   (item 2.7).
4. **Side chats answer only:** no tools, no hand-offs (item 3.5).
5. **Resets, not buying more.** The owner's words: "OpenAI and Claude offer full resets that they
   give out every once in a while. I have one for each right now. If you can't track it then don't
   worry about it." So item 4.2 changes:
   - The notice when a plan runs out says: "If Anthropic gave you a usage reset, you can use it now
     in Claude", with a button that opens the company's own page. The same goes for OpenAI and
     ChatGPT.
   - When the item is built, the builder checks whether Claude Code or Codex reports a waiting
     reset. If one does, Plenipo shows "You have a reset waiting" on that AI tool's card. If
     neither does, Plenipo only reminds, and never guesses.
   - Plenipo never uses a reset for the owner. Using one is the owner's choice, on the company's own
     site or app.
6. **Stepping down is on by default**, with every step shown and recorded, and a switch to turn it
   off (item 4.5).

## Consequences

- **Easier:**
  - A new user can get from first launch to a running objective with the setup tour alone.
  - Every number that matters is honest across organizations.
  - The owner can see, ask, and stop any worker from where it is shown.
- **Harder:**
  - Three phases are built at once. Each must merge `main` often, and two of them change
    `lib.rs`'s command list, so small merge conflicts there are expected.
  - Wave 2 changes many screens, so many screen tests change with it.
- **Watch for:**
  - Old saved settings must still load after fields leave the screen (2.3, 2.4, 2.6).
  - Moving work to a paid key must respect the switch and the spending caps (4.4, 4.5).
  - Side chats must never reach tools (3.5).
  - Stop all must also hold waiting work, not only running work (3.4).

## Alternatives considered

- **Fold the fixes into Phase 23's waves.** Rejected: they are not about Mac or Linux, and mixing
  them would muddy Phase 23's acceptance.
- **Fill in context size automatically instead of removing it.** Rejected: no AI tool reports it,
  and checking each model by hand (ADR-081 §8) is ongoing upkeep for a setting no built-in role
  uses.
- **Build a tour by hand instead of Driver.js.** Rejected: the owner asked for Driver.js, it is
  small and MIT-licensed, and it works with Plenipo's security settings as they are.
- **Let a manager hire freely whenever it can't find a worker.** Rejected as the default: a quiet
  hire costs plan usage and can pass Free's limits. Asking first keeps the owner on top
  (question 3).
