// Router DTO fixtures for tests.
import type { RoleInfo, RolePolicy, RoutingSnapshot, ToolInfo } from "@plenipo/types";

import { ROLES } from "./orgFixtures";

const T0 = Date.UTC(2026, 8, 26, 15, 0, 0);

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
  effortLevels:
    runtimeId === "codex"
      ? ["minimal", "low", "medium", "high", "xhigh"]
      : ["low", "medium", "high", "xhigh", "max"],
  ...patch,
});

const empty: RolePolicy = {
  models: [],
  needs: [],
  minContextTokens: null,
  neverCompanies: [],
  cost: "any",
  crossCompany: "off",
  efforts: {},
};

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
  };
  return {
    models: [
      {
        id: "m-claude",
        runtimeId: "claude-code",
        name: null,
        label: "Claude Code (default model)",
        features: [],
        contextTokens: null,
        cost: "standard",
        effort: null,
        builtIn: true,
      },
      {
        id: "m-codex",
        runtimeId: "codex",
        name: null,
        label: "Codex (default model)",
        features: [],
        contextTokens: null,
        cost: "standard",
        effort: null,
        builtIn: true,
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
            },
          ),
    ),
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
    options: { onUsageLimit: "wait" },
    apiBilling: false,
    notices: [],
    generatedAt: T0,
  };
}
