import type {
  CostClass,
  CostPreference,
  CrossCompany,
  LimitBehavior,
  ModelFeature,
  ModelInfo,
  RoutingSnapshot,
} from "@plenipo/types";

/** What a model can do, in plain words. */
export const FEATURE_LABEL: Record<ModelFeature, string> = {
  vision: "Sees images",
  imageGeneration: "Makes images",
  computerUse: "Uses a computer",
};

export const FEATURES: ModelFeature[] = ["vision", "imageGeneration", "computerUse"];

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
