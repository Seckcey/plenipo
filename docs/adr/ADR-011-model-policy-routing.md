# ADR-011: Plenipo Router — model registry, role model policies, and explained routing

- **Status:** Accepted (owner, 2026-09-26), with effort selection (§15) and model menus (§16)
  added at the owner's request
- **Date:** 2026-09-26
- **Phase:** 6

> **On screen** (ADR-010, plain words and rank names): model policies are "model choices", the preferred model is the "first
> choice" and fallbacks are "backups", a position that follows its role's policy is "Automatic",
> and providers are "AI companies". This ADR keeps the plan's words, which are also the code's.

## Context

Phase 6 makes model and provider selection configurable by role rather than hard-coded. The
plan asks for a Model Registry, a Provider Registry, a Model Policy Engine (preferred and
fallback models by role, capability requirements, availability and usage checks), a routing
explanation, and a settings UI. The acceptance criteria: changing a role's model preference in
Settings changes the next worker Plenipo launches without modifying coordinator prompts or source
code, and Plenipo clearly explains why a provider/model was selected. The plan forbids assuming
that a marketing name exists or that a provider exposes it programmatically, and puts hidden
provider switching, unsupported model scraping, purchasing, and plan changes out of scope.

Earlier decisions constrain it: subscription sign-ins only, API billing refused, and a usage limit
never switches provider inside a turn (ADR-007); the Ledger is the system of record (ADR-006);
Liaison places organization work through the Workforce directory (ADR-008 §2, ADR-009 §5); and
ADR-009 §8 made each position's runtime an explicit owner choice "until the Router".

## Decision

1. **A new crate, `crates/router` (`plenipo-router`)**, planned by ADR-004. It holds the DTOs,
   the stored configuration and its validation, the engine (a pure function), usage-limit
   derivation, and the `Router` service. Nothing in it names a vendor: AI tools and companies are
   the adapters' data.
2. **Configuration is owner data in the Ledger**: one `routing` setting holding the model
   registry, a policy per role, options, and when the owner last cleared each tool's usage limit.
   Every change is validated, applied in one transaction (`Ledger::update_setting`), and recorded
   as an event carrying the change (`router.model_saved`, `router.model_removed`,
   `router.policy_changed`, `router.options_changed`, `router.limit_cleared`,
   `router.models_added`, `router.policies_added`). No schema migration is needed.
3. **Model Registry.** An entry names the AI tool, the model name that tool accepts (or none: the
   tool's own default), the owner's name for it (the alias, kept apart from the provider's
   name), what it can do (sees images, makes images, uses a computer), its context size, and a
   cost class (economical, standard, premium). Plenipo adds one built-in entry per AI tool — its
   default model — which can be described but not removed. The non-interactive CLIs Plenipo uses
   (ADR-007) do not list models, so no model name is ever assumed: Plenipo shows the models each
   AI tool **reported running** (from the executions) as "seen in use", and the owner adds them.
   Capabilities are the owner's statement; an unmarked model is treated as unable. (Amended by
   §16: model names are chosen from a menu that also offers the models a CLI itself lists.)
4. **Provider Registry** is the AI tools this build has adapters for, with their company,
   installation, sign-in, billing method, and usage-limit state (`ToolInfo`).
5. **Role policy** (every field the plan lists): an ordered model list (first choice, then the
   fallback order); required capabilities; minimum context; AI companies the role never uses; a
   cost preference that orders the registry when the role lists no models; and cross-company
   review (off, prefer, require). "Subscription-only" and "API use allowed/disabled" are global
   and fixed in this phase: API billing stays off (ADR-007 §4), and the engine skips a tool signed
   in with an API key with that reason, so no work can fall back to API billing.
6. **The engine** (`route`) takes a policy, the registry, the tools' live state, the project's
   allowed AI tools, and the runtimes whose work is under review. It orders candidates (the
   list, or the registry by cost), puts other companies first (prefer) or drops the reviewed
   companies (require), and checks each: tool present, company allowed, project allows the tool,
   capabilities and context, installed and signed in with a subscription, not at a usage limit.
   The first that passes is chosen. The decision carries one or two plain sentences and a
   verdict and note for every model considered.
7. **Usage/capacity state is derived from the Ledger**, not kept in memory: a tool whose latest
   usage-limited-or-completed turn hit a usage limit is limited until the reset time it reported
   (`…|<unix time>`, as Claude Code reports it), or for an hour when none was reported, unless a
   later turn on it completed or the owner chose "try again now". The same answer after a restart.
8. **Usage-limit behavior** (global, plan §6): **wait** (default) — once a model is skipped for a
   usage limit, models from other companies are skipped too, so work never moves to another AI
   company because of a limit — or **next choice**, which the owner can pick. Either way the
   choice and its reason are recorded; nothing switches silently.
9. **Positions are Automatic or Fixed.** A position's runtime is now optional: none (stored as
   `auto`, which no adapter may use as its ID) means its role's policy chooses; a runtime means
   the owner fixed it (with an optional model). New positions are Automatic by default; positions
   created before Phase 6 keep the runtime they had (Fixed), so upgrading never moves an existing
   position to another AI company. Only fixed runtimes are checked against the project's allowed
   runtimes when the organization changes; automatic ones are routed within them.
10. **When routing happens.** An on-call worker is routed when a handoff is placed (in the
    directory, before Liaison's transaction records the worker); an unroutable request is refused
    with the reason, which the requester is told. A full-time agent is routed when its
    conversation starts (`org.agent_routed`, `Ledger::route_agent`) and keeps that runtime for the
    conversation, for continuity; a new agent routes again. A full-time agent whose conversation's
    tool is at a usage limit refuses new objectives with the reason instead of switching.
11. **Recording the explanation.** The decision is stored with the worker's task
    (`metadata.workforce.routing`), with `org.worker_spawned`, and with an agent's first turn and
    `org.agent_routed`. The organization snapshot shows every position's next route; workers show
    why they got their model; the activity trail quotes the reason.
12. **Cross-company review.** The work under review is the same-workflow tasks a request
    references, or else the requester's own work; Liaison passes their runtimes to the directory.
13. **Coordinator prompts stay put.** A team roster no longer names the AI tool of automatic
    members, so a coordinator's instructions do not change when a policy does.
14. **Starting policies for built-in roles** (the plan's examples, no model names): Designer
    needs a model that sees and makes images; Code Reviewer and Security Auditor prefer another AI
    company; Documentation Writer prefers economical models; Senior Developer premium ones. Given
    once to a template role that has no policy; the owner's policies are never replaced.
15. **Effort per model and per role choice** (added at acceptance, at the owner's request).
    Effort is how much reasoning a model spends on a turn. Each adapter lists the levels its CLI
    accepts (`RuntimeCapabilities::effort_levels`) and passes the chosen one on every turn:
    Claude Code `--effort <level>` (low, medium, high, xhigh, max), Codex
    `-c model_reasoning_effort=<level>` (low, medium, high, xhigh, max, ultra — the levels its
    models accept; none takes "minimal"). A model the CLI itself lists (§16) carries its own
    levels, which can be fewer (Claude Code's Haiku has no effort setting; Codex's GPT-6-Luna
    stops at max, GPT-5.5 at extra high). A model in the registry has an optional effort (none:
    the tool's default); a role's policy can set its own effort for any model (`efforts`, by
    model ID). The engine picks the role's effort, else the model's, and only a level that model
    accepts (`RuntimeCapabilities::effort_levels_for`); the decision records it
    (`RouteChoice.effort`) and says it in the reason ("It runs at high effort"). Settings offers
    only those levels. A runtime session stores its effort (Ledger schema 5,
    `runtime_sessions.effort`) so every turn of a conversation, including a resumed one, runs at
    the same level; the runtime refuses a level its adapter does not list (Codex itself accepts
    any value). A fixed position runs its model at the model's effort. This is the building block
    for organization-wide policies that assign models and effort to jobs, which a later phase can
    offer as presets over role policies.

16. **Model menus** (added after acceptance, at the owner's request). Wherever the owner chooses a
    model — adding one to the list, fixing a position (hire, a new department's or project's
    lead, the details panel), or starting a conversation from Workers — the model is picked from a
    menu of the chosen AI tool's models instead of typed: the AI tool's default first, then the
    models the CLI itself offers (`RuntimeCapabilities::known_models`, shown as "<AI tool>'s
    models"), then the owner's models (outside the Add dialog), then the models seen in use;
    "Type another name…" remains as a last resort. The owner asked for every frontier model at
    the very least, so each adapter lists what its CLI offers, as of the version checked:
    Claude Code 2.1.283's model families from `claude --model`'s alias list — `fable`, `opus`,
    `sonnet`, `haiku` (Fable 5.1, Opus 5.5, Sonnet 5, Haiku 4.5) — and the models Codex
    0.157.1's own model picker lists — `gpt-6-astra`, `gpt-6-sol`, `gpt-6-luna`, `gpt-5.6-sol`,
    `gpt-5.6-terra`, `gpt-5.6-luna`, `gpt-5.5` — each with the effort levels it accepts. These
    are **offered, never added**: nothing enters the model list unless the owner adds it, so §3's
    rule that no model is assumed to exist in the owner's list still holds, and a name a CLI stops
    accepting fails that worker's task with the CLI's own message. A new CLI version that adds
    models needs its adapter's list updated (until then, "seen in use" and typing cover them).
    Names already in the list are shown but not offered in the Add dialog. The adapters keep the
    vendor knowledge; the Router passes it on (`ToolInfo.known_models`).

## Consequences

- The owner changes which model a role's workers get in Settings, and the next worker follows,
  with its reason recorded; coordinators and source code are untouched.
- Model names and capabilities are the owner's data. Plenipo cannot verify that a model can see
  or make images; it trusts the owner's marking and says so in Settings.
- A Designer, by default, has no eligible model until the owner marks one as able to see and make
  images; its requests are refused with that explanation rather than sent to an unfit model.
- Automatic positions depend on the router's view of readiness: while AI tools are still being
  detected at startup, giving an objective waits for detection first.
- A later success on an AI tool lifts its usage limit (the provider accepted work again); a tool
  that stays limited without a reset time is tried again after an hour.
- Pay-per-use API billing remains off; enabling it later needs adapter changes (ADR-007) as well
  as a setting.
- Effort levels are the CLIs' own; a new CLI version that adds or drops a level needs its
  adapter's list updated. A role's effort for a model that moves to another AI tool is dropped
  when that tool does not accept it.

## Alternatives considered

- **Tables for models and policies (a migration)** — more schema for what is owner
  configuration; the settings document is validated in Rust and changed atomically, with events.
- **Keeping usage limits in memory** — lost on restart, or needing its own persistence; deriving
  them from recorded turn results needs neither.
- **Moving every existing position to its role's policy** — an upgrade could silently move a
  position to another AI company, which the plan rules out.
- **Routing a full-time agent on every objective** — would break its conversation whenever a
  policy or a tool's state changed.
- **Seeding named models (aliases such as "opus")** — the plan forbids assuming marketing names;
  the owner adds the names their tools accept. (§16 offers the models a CLI itself lists in the
  model menus, but still adds nothing on the owner's behalf.)
- **Switching company on a usage limit by default** — the plan's own option is to pause, and
  earlier phases promised that a usage limit never moves work to another AI company.
- **Effort as part of the model entry only** — the same model often suits several jobs at
  different efforts (a quick reviewer, a careful architect); a per-role setting avoids duplicate
  registry entries.
