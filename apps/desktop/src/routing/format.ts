import type {
  CostClass,
  CostPreference,
  CrossCompany,
  Effort,
  KnownModel,
  LimitBehavior,
  Maker,
  ModelInfo,
  ModelRule,
  RouteChoice,
  RouteDecision,
  RuleLayer,
  RuleSource,
  RoutingSnapshot,
} from "@plenipo/types";

import { priceWords } from "../spending/words";

/** How hard a model thinks before it answers. */
export const EFFORT_LABEL: Record<Effort, string> = {
  minimal: "Minimal",
  low: "Low",
  medium: "Medium",
  high: "High",
  xhigh: "Extra high",
  max: "Max",
  ultra: "Ultra",
};

/**
 * The effort levels a model accepts: a model the AI tool lists has its own (none: no effort
 * setting); any other model, or the AI tool's default, has the AI tool's.
 */
export function effortLevels(
  snapshot: RoutingSnapshot,
  runtimeId: string,
  name?: string | null,
): Effort[] {
  const tool = snapshot.tools.find((t) => t.runtimeId === runtimeId);
  const known = name ? tool?.knownModels.find((k) => k.name === name) : undefined;
  return known?.effortLevels ?? tool?.effortLevels ?? [];
}

/** One group of model names an AI tool can run, in the order the model menus show them. */
export interface ModelGroup {
  label: string;
  options: { name: string; label: string }[];
}

/**
 * The model names to offer for `runtimeId`, after "the AI tool's default": the models the tool
 * itself offers (those it reported that Plenipo has not checked last, marked "new, not checked
 * yet", ADR-060 §5), then (with `yours`) the owner's models, then the names it reported running.
 * Each name appears once. Choosing a new model gives the AI tool that name, as a typed one would.
 */
export function modelGroups(
  snapshot: RoutingSnapshot | null,
  runtimeId: string,
  { yours = true }: { yours?: boolean } = {},
): ModelGroup[] {
  if (!snapshot) return [];
  const tool = snapshot.tools.find((t) => t.runtimeId === runtimeId);
  const shown = new Set<string>();
  const group = (label: string, options: { name: string; label: string }[]): ModelGroup => ({
    label,
    options: options.filter((o) => !shown.has(o.name) && shown.add(o.name)),
  });
  // Who made a name the tool's list does not name: its company, for an AI tool that runs only
  // its own company's models; not known for one that runs other companies' too (ADR-081 §2).
  const unlisted = companyOnly(tool);
  // "opus — now Opus 5.5 — made by Anthropic" (ADR-081 §6, §8).
  const words = (k: KnownModel) => {
    const parts = [k.name];
    const now = k.pointsTo && tool?.knownModels.find((x) => x.name === k.pointsTo);
    if (now) parts.push(`now ${now.label}`);
    parts.push(madeByWords(k.maker ?? unlisted));
    if (k.price) parts.push(priceWords(k.price));
    return parts.join(" — ");
  };
  const paid = tool?.auth === "paidKey" ? " (paid per use)" : "";
  const groups = [
    group(`${tool?.label ?? "The AI tool"}'s models${paid}`, [
      ...(tool?.knownModels ?? []).map((k) => ({ name: k.name, label: words(k) })),
      ...(tool?.newModels ?? []).map((k) => ({
        name: k.name,
        label: `${words(k)} — new, not checked yet`,
      })),
    ]),
    group(
      "Your models",
      yours
        ? snapshot.models.flatMap((m) =>
            m.runtimeId === runtimeId && m.name
              ? [
                  {
                    name: m.name,
                    label: `${m.label === m.name ? m.name : `${m.label} — ${m.name}`} — ${madeByWords(m.maker)}`,
                  },
                ]
              : [],
          )
        : [],
    ),
    group(
      "Seen in use",
      snapshot.seen
        .filter((s) => s.runtimeId === runtimeId)
        .map((s) => ({ name: s.name, label: `${s.name} — ${madeByWords(unlisted)}` })),
    ),
  ];
  return groups.filter((g) => g.options.length > 0);
}

/** "Opus (Claude Code) · high effort". */
export function choiceLabel(choice: RouteChoice): string {
  return choice.effort
    ? `${choice.label} · ${EFFORT_LABEL[choice.effort].toLowerCase()} effort`
    : choice.label;
}

export const COST_LABEL: Record<CostClass, string> = {
  economical: "Economical",
  standard: "Standard",
  premium: "Premium",
};

export const COSTS: CostClass[] = ["economical", "standard", "premium"];

export const COST_PREFERENCE_LABEL: Record<CostPreference, string> = {
  any: "In the order of your list",
  economical: "Economical models first",
  premium: "Premium models first",
};

export const CROSS_COMPANY_LABEL: Record<CrossCompany, string> = {
  off: "No preference",
  prefer: "Prefer a different AI company",
  require: "Only a different AI company",
};

export const LIMIT_LABEL: Record<LimitBehavior, string> = {
  wait: "Wait for the limit to reset — never move the work to another AI company",
  nextChoice: "Use the role's next choice, even from another AI company",
};

/** "Opus (Claude Code)", or the model's own name when it already names its AI tool. */
export function modelLabel(snapshot: RoutingSnapshot, model: ModelInfo): string {
  const tool =
    snapshot.tools.find((t) => t.runtimeId === model.runtimeId)?.label ?? model.runtimeId;
  return model.label.toLowerCase().includes(tool.toLowerCase())
    ? model.label
    : `${model.label} (${tool})`;
}

/** One model a model list can add: one in Your models (`id:…`), or one to add to it (`add:…`). */
export interface AddableModel {
  value: string;
  label: string;
}

export interface AddableGroup {
  label: string;
  options: AddableModel[];
}

/** The menu value for a model of `runtimeId`'s that is not in Your models yet. */
export function addValue(runtimeId: string, name: string): string {
  return `add:${runtimeId}:${name}`;
}

/** What a menu value stands for. */
export function readAddValue(
  value: string,
): { id: string } | { runtimeId: string; name: string } | null {
  if (value.startsWith("id:")) return { id: value.slice(3) };
  const m = /^add:([^:]+):(.+)$/.exec(value);
  return m ? { runtimeId: m[1] as string, name: m[2] as string } : null;
}

/**
 * Every model of every AI tool, for a list of models (Phase 25, item 2.5): one group per AI
 * tool, the subscriptions first, then each paid AI tool whose key works. In each group: its
 * models already in Your models (its own choice first), then the models it offers that aren't
 * yet, which picking adds to Your models. `listed`: model IDs already on the list, left out.
 */
export function everyModel(snapshot: RoutingSnapshot, listed: readonly string[]): AddableGroup[] {
  const tools = [
    ...snapshot.tools.filter((t) => !t.paid),
    ...snapshot.tools.filter((t) => t.paid && t.ready),
  ];
  const groups: AddableGroup[] = [];
  for (const t of tools) {
    const yours = snapshot.models
      .filter((m) => m.runtimeId === t.runtimeId)
      .sort((a, b) => Number(b.builtIn) - Number(a.builtIn));
    const options: AddableModel[] = yours
      .filter((m) => !listed.includes(m.id))
      .map((m) => ({ value: `id:${m.id}`, label: modelLabel(snapshot, m) }));
    const have = new Set(yours.map((m) => m.name ?? ""));
    for (const k of [...t.knownModels, ...t.newModels]) {
      if (have.has(k.name)) continue;
      have.add(k.name);
      const same = k.label.toLowerCase() === k.name.toLowerCase();
      options.push({
        value: addValue(t.runtimeId, k.name),
        label: same ? k.label : `${k.label} (${k.name})`,
      });
    }
    if (options.length > 0) {
      groups.push({ label: t.paid ? `${t.label} (paid per use)` : t.label, options });
    }
  }
  // Your models whose AI tool isn't offered here (a paid AI tool whose key doesn't work now).
  const shown = new Set(tools.map((t) => t.runtimeId));
  const rest = snapshot.models.filter((m) => !shown.has(m.runtimeId) && !listed.includes(m.id));
  if (rest.length > 0) {
    groups.push({
      label: "Your other models",
      options: rest.map((m) => ({ value: `id:${m.id}`, label: modelLabel(snapshot, m) })),
    });
  }
  return groups;
}

/**
 * A name for a model being added to Your models that no model there has yet: its own ("Sonnet"),
 * then with its AI tool ("Sonnet (Claude Code)"), then numbered.
 */
export function freeModelLabel(snapshot: RoutingSnapshot, base: string, tool: string): string {
  const taken = (l: string) =>
    snapshot.models.some((m) => m.label.toLowerCase() === l.toLowerCase());
  if (!taken(base)) return base;
  const withTool = `${base} (${tool})`;
  if (!taken(withTool)) return withTool;
  for (let n = 2; ; n++) if (!taken(`${withTool} ${n}`)) return `${withTool} ${n}`;
}

/** "1.2M", "200K", "8,000". */
export function tokens(n: number): string {
  if (n >= 1_000_000) return `${+(n / 1_000_000).toFixed(1)}M`;
  if (n >= 10_000) return `${Math.round(n / 1000)}K`;
  return n.toLocaleString("en-US");
}

/** "in 25 min", "in 3 h", "now". */
export function until(ms: number, now: number = Date.now()): string {
  const mins = Math.max(0, Math.ceil((ms - now) / 60_000));
  if (mins <= 0) return "now";
  if (mins < 60) return `in ${mins} min`;
  const hours = Math.round(mins / 60);
  if (hours < 48) return `in ${hours} h`;
  return `in ${Math.round(hours / 24)} d`;
}

/**
 * Every AI company this version of Plenipo knows, by name: the AI tools' own and the companies
 * that made the models they list (ADR-081 §5). "AI companies never to use" offers these.
 */
export function companies(snapshot: RoutingSnapshot): Maker[] {
  return snapshot.companies;
}

/** Who made a model, as a column or heading: its company's name, or "Not known" (ADR-081 §7). */
export function makerWords(maker: Maker | null | undefined): string {
  return maker?.label ?? "Not known";
}

/** Who made a model, inside a sentence: "made by DeepSeek", or "who made it is not known". */
export function madeByWords(maker: Maker | null | undefined): string {
  return maker ? `made by ${maker.label}` : "who made it is not known";
}

/**
 * Who made any model an AI tool runs, when that is its own company (it runs no other company's
 * models); `null` for an AI tool that runs other companies' models too, or none.
 */
export function companyOnly(
  tool: { company: string; companyLabel: string; runsOtherMakers: boolean } | undefined,
): Maker | null {
  return tool && !tool.runsOtherMakers ? { id: tool.company, label: tool.companyLabel } : null;
}

/** How the model list is grouped (ADR-081 §6). */
export type GroupBy = "maker" | "tool";

export const GROUP_BY_LABEL: Record<GroupBy, string> = {
  maker: "Who made it",
  tool: "AI tool",
};

export function isGroupBy(v: unknown): v is GroupBy {
  return v === "maker" || v === "tool";
}

/** One heading of the grouped model list and its models, in the list's own order. */
export interface ModelListGroup {
  key: string;
  label: string;
  models: ModelInfo[];
}

/**
 * The owner's models grouped by who made them or by the AI tool that runs them (ADR-081 §6).
 * Both groupings hold exactly the same models; groups are in name order, a model whose maker is
 * not known last, and each group keeps the list's order.
 */
export function groupModels(snapshot: RoutingSnapshot, by: GroupBy): ModelListGroup[] {
  const groups = new Map<string, ModelListGroup>();
  for (const m of snapshot.models) {
    const tool = snapshot.tools.find((t) => t.runtimeId === m.runtimeId);
    const [key, label] =
      by === "tool"
        ? [m.runtimeId, tool?.label ?? m.runtimeId]
        : [m.maker?.id ?? "", makerWords(m.maker)];
    const group = groups.get(key) ?? { key, label, models: [] };
    group.models.push(m);
    groups.set(key, group);
  }
  return [...groups.values()].sort((a, b) =>
    a.key === "" ? 1 : b.key === "" ? -1 : a.label.localeCompare(b.label),
  );
}

// ---- Model and effort rules (Phase 17, ADR-041) ----------------------------------------------

/** Every effort level, lowest first. */
export const EFFORTS: Effort[] = ["minimal", "low", "medium", "high", "xhigh", "max", "ultra"];

/** A rule that sets nothing. */
export function emptyRule(): ModelRule {
  return { models: [], efforts: {}, effort: null, neverCompanies: [] };
}

/** Sets nothing (so saving it for a department or an agent removes it). */
export function isEmptyRule(rule: ModelRule): boolean {
  return (
    rule.models.length === 0 &&
    Object.keys(rule.efforts).length === 0 &&
    rule.effort === null &&
    rule.neverCompanies.length === 0
  );
}

/** The effort levels at least one AI tool takes, lowest first. */
export function anyEffortLevels(snapshot: RoutingSnapshot): Effort[] {
  const taken = new Set<Effort>();
  for (const t of snapshot.tools) {
    t.effortLevels.forEach((e) => taken.add(e));
    t.knownModels.forEach((k) => k.effortLevels.forEach((e) => taken.add(e)));
  }
  return EFFORTS.filter((e) => taken.has(e));
}

/** Who decided, in the words the reasons use. */
export const RULE_LAYER_LABEL: Record<RuleLayer, string> = {
  fixed: "you fixed it for this agent",
  agent: "its own rule",
  role: "its role's rule",
  department: "its department's rule",
  organization: "the organization's rule",
  model: "the model's own setting",
};

/** "Opus (Claude Code), then Codex: its own choice · high effort · never OpenAI". */
export function ruleSummary(snapshot: RoutingSnapshot, rule: ModelRule): string {
  const byId = new Map(snapshot.models.map((m) => [m.id, m]));
  const name = (id: string) => {
    const m = byId.get(id);
    return m ? modelLabel(snapshot, m) : "a removed model";
  };
  const parts: string[] = [];
  if (rule.models.length > 0) parts.push(rule.models.map(name).join(", then "));
  if (rule.effort) parts.push(`${EFFORT_LABEL[rule.effort].toLowerCase()} effort`);
  for (const [id, effort] of Object.entries(rule.efforts)) {
    if (effort) parts.push(`${EFFORT_LABEL[effort].toLowerCase()} effort for ${name(id)}`);
  }
  if (rule.neverCompanies.length > 0) {
    const label = (id: string) => snapshot.companies.find((c) => c.id === id)?.label ?? id;
    parts.push(`never ${rule.neverCompanies.map(label).join(" or ")}`);
  }
  return parts.length > 0 ? parts.join(" · ") : "Sets nothing";
}

/** Where a setting came from, as the reasons say it ("Senior Developer's rule"). */
export function sourceWords(source: RuleSource): string {
  switch (source.layer) {
    case "agent":
      return "this agent's own setting";
    case "model":
    case "fixed":
      return "the model's own setting";
    default:
      return `${source.name}'s rule`;
  }
}

/** Where a role's next worker's model came from, in plain words (Phase 25, item 2.6). */
export function modelFromWords(route: RouteDecision): string | null {
  const from = route.modelFrom;
  if (route.fixed) return "fixed on the position";
  switch (from?.layer) {
    case "agent":
      return "from its own rule";
    case "role":
      return `from ${from.name}'s choices`;
    case "department":
      return `from the ${from.name} department`;
    case "organization":
      return "from the whole organization";
    default:
      return route.choice ? "from Your models (nothing is listed for it)" : null;
  }
}

/** "High effort, from Senior Developer's rule", or null when the AI tool's default applies. */
export function effortLine(route: RouteDecision | null): string | null {
  const effort = route?.choice?.effort;
  if (!route || !effort) return null;
  const from = route.effortFrom ? `, from ${sourceWords(route.effortFrom)}` : "";
  return `${EFFORT_LABEL[effort]} effort${from}`;
}
