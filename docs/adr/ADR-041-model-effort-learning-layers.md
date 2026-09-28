# ADR-041: Model, effort, and learning set in layers — organization, department, role, agent

- **Status:** Proposed (2026-09-28), waiting for the owner
- **Date:** 2026-09-28
- **Phase:** 17
- **Amends:** ADR-011 (the Router: model choices and routing) §5, §9, §11, and §15; ADR-024
  (workers learn from their work), its one switch

> **On screen** (ADR-010, plain words and rank names): a layer's settings are its **rule** ("the
> Development department's rule"). Effort is **effort (how hard the model thinks)**, a fixed
> position is **fixed**, and a position that follows the rules is **Automatic**. An agent is a
> position on the chart. This record keeps the plan's words, which are also the code's.

## In short

You can set which AI models your workers use, and how hard they think, for the whole
organization, for one department, for one role, or for one agent. The setting closest to the
agent wins. Plenipo's "why this model" always says which setting decided. Learning can be turned
on or off the same way. Accepting this record means building it as written below.

## Context

Phase 17 of `ROLLOUT_PLAN.md` asks for:

- **effort per agent:** "a position can set its effort, with or without fixing its AI tool and
  model";
- **model and effort rules in layers:** "organization → department → role → agent, and the
  closest layer that sets something wins. Each layer can set an ordered list of models, the effort
  for each, and AI companies never to use";
- **learning in layers:** "on or off for the organization (today's switch), for each role, and
  for each agent, the closest winning; each role's 'keep lessons without asking' stays";
- in the Router, the order "fixed agent → agent's own settings → role → department →
  organization → model default", with a reason that "names the layer that decided ('Effort high,
  from the Development department's rule')";
- "changing only effort does not hire a new agent", while changing a position's model keeps
  today's warning that it does.

What the code does today (v1.9.0, read at `b199e5d`):

- **The routing setting** (`crates/router/src/config.rs`) holds your model list, one policy per
  role (`RolePolicy`: models in order, an effort per model, what the model must be able to do, the
  smallest context, AI companies never to use, cheapest or dearest first, and cross-company
  review), and the options. Departments and the organization have nothing.
- **A position** is Automatic (its role's policy picks) or fixed (an AI tool and, optionally, a
  model). A fixed position runs at its model's own effort. There is no effort on a position.
- **The engine** (`crates/router/src/engine.rs`) walks the role's list, skips what cannot run,
  and explains its choice. Effort comes from the role's setting for that model, else the model's
  own setting.
- **A conversation keeps its effort.** Each full-time agent's conversation records its effort
  (Ledger layout 5, `runtime_sessions.effort`), and every task in it runs at that level. The AI
  tools take the level with each task (Claude Code `--effort`, Codex
  `-c model_reasoning_effort=…`), so a new level can take effect at the next task.
- **Learning** (`crates/workforce/src/learning.rs`) is one switch for everything (Settings →
  Switches → Worker learning) and a list of roles that keep lessons without asking.

## Decision

### Model and effort

1. **Four layers**, from the widest to the closest: **organization → department → role →
   agent**. Every layer can set:
   - an **ordered list of models** (first choice, then backups);
   - an **effort for each model** in that list;
   - an **effort for any other model** (new: so you can say "high effort for everything" without
     listing models);
   - **AI companies never to use**.

   A role keeps its other choices — what the model must be able to do, the smallest context,
   cheapest or dearest first, and reviews by another AI company — because those describe the
   job, not where it sits.

2. **The closest layer wins, one setting at a time:**
   - **The model list:** the closest layer that lists models decides the whole list. Lists are
     never merged. With no list anywhere, Plenipo uses all your models, in the role's cost order
     (as today).
   - **The effort for the chosen model:** from the closest layer that sets one for it — that
     layer's effort for this model, else its effort for any model. With none, the model's own
     setting, else the AI tool's default (as today). An effort the chosen model does not take is
     passed over, and the reason says so ("… ultra, set for this agent, does not work with Opus").
   - **AI companies never to use add up** (see §4): a company any layer names is never used for
     that agent.

3. **The order** is the plan's: **fixed agent → the agent's own rule → role → department →
   organization → the model's own setting.** A fixed agent's AI tool and model win over every
   list. Its effort still comes from the layers, its own first.

4. **Never-use lists add up; they are not "closest wins".** This is the one place this record
   departs from the plan's words. If "closest wins" applied here, a department that said "never
   use OpenAI" would silently allow a company the whole organization had ruled out. "Never" has
   to mean never. So:
   - a company named by any layer above an agent, or by the agent itself, is skipped, and the
     reason names the rule ("the organization never uses xAI");
   - a fixed agent whose AI company a layer rules out cannot start, and Plenipo says which rule
     blocks it; saving such a fixed choice is refused with the same words.

5. **An agent's department** is the department it works in: the department its manager runs, or
   its project's department. A position outside any department (a VP reporting to you) skips the
   department layer.

6. **The reason names the layer that decided.** Examples:
   - "Opus (Claude Code) is the Development department's first choice and is ready. It runs at
     high effort, from the Development department's rule."
   - "Sonnet (Claude Code) is the organization's first choice and is ready. It runs at max
     effort, from this agent's own setting."
   - "You set Code Reviewer to always use GPT-6-Sol (Codex). It runs at high effort, from the
     organization's rule."

   The decision also records the layer of each (`modelFrom`, `effortFrom`), so screens can show
   it and tests can check it.

7. **Effort alone never hires a new agent.** Changing only an agent's effort (or any effort in a
   rule) keeps the agent. A full-time agent's open conversation takes the new level from its
   **next** task: Plenipo updates the level its conversation records, and says so in the
   agent's activity. Changing the AI tool or model of a staffed full-time agent still hires a new
   agent, with today's warning.

8. **Checked when you save.** An effort a model does not take is refused, in plain words, naming
   the levels it does take: "Sonnet (Claude Code) does not take ultra effort. It takes low,
   medium, high, extra high, or max." A model with no effort setting: "Haiku (Claude Code) has no
   effort setting." An effort for any model must be one at least one of your AI tools takes.

9. **Where it is kept.** In the same "routing" setting as today's role choices (ADR-011 §2): the
   organization's rule, each department's rule, and each agent's own rule, changed in one
   transaction and recorded as `router.rule_changed` with the layer, its name, and the new rule.
   No change to the Ledger's layout is needed for this. When a department or an agent is deleted
   for good (ADR-043), its rule goes with it.

### Learning

10. **The organization's switch stays the main switch** (Settings → Switches → Worker learning):
    off stops all learning, as today. The plan's test says so: "learning off at the organization
    stops all learning".
11. **Under it, each role and each agent can be on or off, and the closest wins.** A role is on
    unless you turn it off. An agent follows its role unless you set it on or off. So an agent set
    to on, inside a role set to off, learns.
12. **Off means what it means today** (ADR-024): the agent neither writes down new lessons nor
    gets its role's kept lessons. Lessons stay per role.
13. **Each role's "Learn on its own"** (keep its lessons without asking) stays exactly as it is.
14. Kept in the "learning" setting, recorded as `learning.role_switched` and
    `learning.agent_switched`. An agent deleted for good loses its setting.

### Where you set it

15. **Settings → AI models → Model and effort rules:** the whole organization, each department,
    each role (today's table), and the agents that have their own rule, each with what its next
    worker gets and why.
16. **Each agent's panel:** the AI model tab has its AI tool and model (Automatic or fixed), its
    effort, and its own rule; the Job tab has its learning (follow the role, on, or off) and its
    role's.

## Consequences

- **More places to set the same thing,** so the answer to "why this model?" must always be on
  screen. Every reason names the layer, and the panel shows it next to the model and effort.
- **A role's list beats a department's list,** because the plan puts the role closer to the
  agent. If a role lists its own models, a department's list does not reach that role's workers.
  The built-in roles list no models, so department and organization lists reach them unless you
  give a role its own list. The reason makes this visible.
- **Full-time agents change effort without starting over.** Their conversation, and what they
  remember in it, is kept.
- **"Never" means never.** Lifting a ban set for the whole organization is done where it was set.
- **Nothing is moved on upgrade.** With no new rules, every worker gets exactly what it gets
  today.

## Alternatives considered

- **Closest wins for never-use lists too** (the plan's words, taken literally). Rejected (§4): a
  department's list would silently drop the organization's ban.
- **The department closer than the role.** Not chosen: the plan sets the order, and a job's needs
  (a Designer needs a model that sees images) should not be undone by where the job sits.
- **Merging lists from several layers.** Rejected: nobody could predict the order.
- **Effort per model only, with no "any model" effort.** Rejected: to say "everything at high
  effort" you would have to list every model at every layer.
- **Separate presets on top of role choices** (ADR-011 §15's idea). Not needed: the layers are
  those presets.
- **Hiring a new agent when effort changes,** as for a new model. Rejected by the plan, and not
  needed: the AI tools take a new level at each task.
