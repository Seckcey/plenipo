import type {
  CostClass,
  CostPreference,
  CrossCompany,
  Effort,
  LimitBehavior,
  ModelFeature,
  ModelInfo,
  ModelRule,
  RouteChoice,
  RouteDecision,
  RuleLayer,
  RuleSource,
  RoutingSnapshot,
} from "@plenipo/types";

/** What a model can do, in plain words. */
export const FEATURE_LABEL: Record<ModelFeature, string> = {
  vision: "Sees images",
  imageGeneration: "Makes images",
  computerUse: "Uses a computer",
};

export const FEATURES: ModelFeature[] = ["vision", "imageGeneration", "computerUse"];

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
  const groups = [
    group(`${tool?.label ?? "The AI tool"}'s models`, [
      ...(tool?.knownModels ?? []).map((k) => ({ name: k.name, label: k.name })),
      ...(tool?.newModels ?? []).map((k) => ({
        name: k.name,
        label: `${k.name} — new, not checked yet`,
      })),
    ]),
    group(
      "Your models",
      yours
        ? snapshot.models.flatMap((m) =>
            m.runtimeId === runtimeId && m.name
              ? [{ name: m.name, label: m.label === m.name ? m.name : `${m.label} — ${m.name}` }]
              : [],
          )
        : [],
    ),
    group(
      "Seen in use",
      snapshot.seen
        .filter((s) => s.runtimeId === runtimeId)
        .map((s) => ({ name: s.name, label: s.name })),
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

/** Each AI company once, in the AI tools' order. */
export function companies(snapshot: RoutingSnapshot): { id: string; label: string }[] {
  const out: { id: string; label: string }[] = [];
  for (const t of snapshot.tools) {
    if (!out.some((c) => c.id === t.company)) out.push({ id: t.company, label: t.companyLabel });
  }
  return out;
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

/** "Opus (Claude Code), then Codex (default model) · high effort · never OpenAI". */
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
    const label = (id: string) => snapshot.tools.find((t) => t.company === id)?.companyLabel ?? id;
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

/** "High effort, from Senior Developer's rule", or null when the AI tool's default applies. */
export function effortLine(route: RouteDecision | null): string | null {
  const effort = route?.choice?.effort;
  if (!route || !effort) return null;
  const from = route.effortFrom ? `, from ${sourceWords(route.effortFrom)}` : "";
  return `${EFFORT_LABEL[effort]} effort${from}`;
}
