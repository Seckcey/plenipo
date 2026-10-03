// Router DTO fixtures for tests.
import type {
  Effort,
  ModelRule,
  RoleInfo,
  RolePolicy,
  RoutingSnapshot,
  ToolInfo,
} from "@plenipo/types";

import { ROLES } from "./orgFixtures";

const T0 = Date.UTC(2026, 8, 26, 15, 0, 0);

const MAX: Effort[] = ["low", "medium", "high", "xhigh", "max"];
const ULTRA: Effort[] = [...MAX, "ultra"];

export const tool = (runtimeId: string, patch: Partial<ToolInfo> = {}): ToolInfo => ({
  runtimeId,
  label: runtimeId === "codex" ? "Codex" : "Claude Code",
  company: runtimeId === "codex" ? "openai" : "anthropic",
  companyLabel: runtimeId === "codex" ? "OpenAI" : "Anthropic",
  ready: true,
  auth: "subscription",
  status: "Ready: signed in with a subscription",
  usageLimit: null,
  available: true,
  paid: false,
  effortLevels: runtimeId === "codex" ? ULTRA : MAX,
  knownModels:
    runtimeId === "codex"
      ? [
          { name: "gpt-6-sol", label: "GPT-6-Sol", effortLevels: ULTRA },
          { name: "gpt-6-luna", label: "GPT-6-Luna", effortLevels: MAX },
        ]
      : [
          { name: "fable", label: "Fable", effortLevels: MAX },
          { name: "opus", label: "Opus", effortLevels: MAX },
          { name: "sonnet", label: "Sonnet", effortLevels: MAX },
          { name: "haiku", label: "Haiku", effortLevels: [] },
        ],
  newModels: [],
  unlistedModels: [],
  runsOtherMakers: false,
  ...patch,
});

export const ANTHROPIC = { id: "anthropic", label: "Anthropic" };
export const OPENAI = { id: "openai", label: "OpenAI" };

const empty: RolePolicy = {
  models: [],
  needs: [],
  minContextTokens: null,
  neverCompanies: [],
  cost: "any",
  crossCompany: "off",
  efforts: {},
  effort: null,
};

/** A rule that sets nothing. */
export const emptyRule = (): ModelRule => ({
  models: [],
  efforts: {},
  effort: null,
  neverCompanies: [],
});

/** Two built-in defaults and the owner's "Opus"; Senior Developer prefers Opus, then Codex. */
export function sampleRouting(): RoutingSnapshot {
  const view = (r: RoleInfo, policy: RolePolicy, next: RoutingSnapshot["roles"][0]["next"]) => ({
    roleId: r.id,
    roleName: r.name,
    fullTime: r.staffing === "persistent",
    policy,
    next,
  });
  const choice = {
    modelId: "m-opus",
    runtimeId: "claude-code",
    runtimeLabel: "Claude Code",
    company: "anthropic",
    model: "opus",
    effort: null,
    label: "Opus (Claude Code)",
    paid: false,
  };
  return {
    models: [
      {
        id: "m-claude",
        runtimeId: "claude-code",
        name: null,
        label: "Claude Code: its own choice",
        features: [],
        contextTokens: null,
        cost: "standard",
        effort: null,
        builtIn: true,
        maker: ANTHROPIC,
      },
      {
        id: "m-codex",
        runtimeId: "codex",
        name: null,
        label: "Codex: its own choice",
        features: [],
        contextTokens: null,
        cost: "standard",
        effort: null,
        builtIn: true,
        maker: OPENAI,
      },
      {
        id: "m-opus",
        runtimeId: "claude-code",
        name: "opus",
        label: "Opus",
        features: ["vision"],
        contextTokens: 200_000,
        cost: "premium",
        effort: null,
        builtIn: false,
        maker: ANTHROPIC,
      },
    ],
    tools: [
      tool("claude-code"),
      tool("codex", {
        available: false,
        status: "Codex reached its usage limit (resets in about 2 hours)",
        usageLimit: {
          model: null,
          since: T0,
          resetsAt: T0 + 7_200_000,
          until: T0 + 7_200_000,
          detail: "You've hit your usage limit.",
        },
      }),
    ],
    roles: ROLES.filter((r) => r.name === "Senior Developer" || r.name === "Designer").map((r) =>
      r.name === "Senior Developer"
        ? view(
            r,
            { ...empty, models: ["m-opus", "m-codex"] },
            {
              choice,
              reason: "Opus (Claude Code) is Senior Developer's first choice and is ready.",
              rank: 1,
              candidates: [],
              fixed: false,
              modelFrom: { layer: "role", name: "Senior Developer", id: r.id },
              effortFrom: null,
            },
          )
        : view(
            r,
            { ...empty, needs: ["vision", "imageGeneration"] },
            {
              choice: null,
              reason:
                "No model can take Designer's work now — Opus (Claude Code): it is not marked as able to make images.",
              rank: null,
              candidates: [],
              fixed: false,
              modelFrom: null,
              effortFrom: null,
            },
          ),
    ),
    organization: emptyRule(),
    departments: [
      { departmentId: "d-eng", name: "Engineering", rule: emptyRule() },
      { departmentId: "d-mkt", name: "Marketing", rule: emptyRule() },
    ],
    agents: [],
    seen: [
      {
        runtimeId: "claude-code",
        runtimeLabel: "Claude Code",
        name: "claude-opus-5-5",
        runs: 3,
        lastUsed: T0,
        listed: false,
      },
    ],
    companies: [ANTHROPIC, OPENAI],
    options: { onUsageLimit: "wait", stepDown: true, stepDownAt: 80 },
    apiBilling: false,
    notices: [],
    generatedAt: T0,
  };
}
