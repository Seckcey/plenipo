// AI tools page fixtures (Phase 19): the seven AI tools, as their checks and the page report them.
import type {
  AgentRuntimeInfo,
  AiToolsPage,
  AiToolState,
  AiToolUpdate,
  KnownModel,
  ToolInfo,
} from "@plenipo/types";

import { tool as routedTool } from "./routingFixtures";

export const T0 = Date.UTC(2026, 8, 28, 20, 0, 0);

const MAX = ["low", "medium", "high", "xhigh", "max"] as const;

const MODELS: Record<string, KnownModel[]> = {
  "claude-code": [
    { name: "opus", label: "Opus", effortLevels: [...MAX] },
    { name: "haiku", label: "Haiku", effortLevels: [] },
  ],
  codex: [
    { name: "gpt-6-sol", label: "GPT-6-Sol", effortLevels: [...MAX, "ultra"] },
    { name: "gpt-6-luna", label: "GPT-6-Luna", effortLevels: [...MAX] },
  ],
  grok: [{ name: "grok-4", label: "Grok 4", effortLevels: [] }],
  kimi: [{ name: "kimi-code/k2", label: "K2", effortLevels: [] }],
  ollama: [],
  antigravity: [
    {
      name: "gemini-3.1-pro-high",
      label: "Gemini 3.1 Pro (High)",
      effortLevels: [],
      maker: { id: "google", label: "Google" },
    },
  ],
  // Only Auto on the owner's plan, Copilot's default: no model is listed (ADR-083 §5).
  copilot: [],
  // A paid AI tool's starter list, each model priced (ADR-085).
  openrouter: [
    {
      name: "qwen/qwen3.8-flash",
      label: "Qwen3.8 Flash",
      effortLevels: [],
      maker: { id: "alibaba", label: "Alibaba (Qwen)" },
      price: { input: 3_000_000, cachedInput: null, output: 15_000_000 },
    },
  ],
};

const FACTS: Record<
  string,
  {
    label: string;
    provider: string;
    providerLabel: string;
    signIn: string | null;
    signOut: string | null;
  }
> = {
  "claude-code": {
    label: "Claude Code",
    provider: "anthropic",
    providerLabel: "Anthropic",
    signIn: "claude auth login",
    signOut: "claude auth logout",
  },
  codex: {
    label: "Codex",
    provider: "openai",
    providerLabel: "OpenAI",
    signIn: "codex login",
    signOut: "codex logout",
  },
  grok: {
    label: "Grok",
    provider: "xai",
    providerLabel: "xAI",
    signIn: "grok login",
    signOut: "grok logout",
  },
  kimi: {
    label: "Kimi",
    provider: "moonshot",
    providerLabel: "Moonshot AI",
    signIn: "kimi login",
    signOut: null,
  },
  ollama: {
    label: "Ollama",
    provider: "ollama",
    providerLabel: "Ollama",
    signIn: "ollama signin",
    signOut: "ollama signout",
  },
  antigravity: {
    label: "Antigravity",
    provider: "google",
    providerLabel: "Google",
    signIn: "agy",
    signOut: null,
  },
  copilot: {
    label: "GitHub Copilot",
    provider: "github",
    providerLabel: "GitHub",
    signIn: "copilot login",
    signOut: null,
  },
  // Nothing to sign in to: the key is added on its card.
  openrouter: {
    label: "OpenRouter",
    provider: "openrouter",
    providerLabel: "OpenRouter",
    signIn: null,
    signOut: null,
  },
};

export const AI_TOOL_IDS = [
  "claude-code",
  "codex",
  "grok",
  "kimi",
  "ollama",
  "antigravity",
  "copilot",
] as const;

/** The AI tools that run other companies' models (ADR-081). */
const OTHER_MAKERS = new Set(["ollama", "antigravity", "copilot", "openrouter"]);

/** The paid AI tools (ADR-085): they come with Plenipo and are paid per use with a key. */
const PAID = new Set(["openrouter"]);

/** An AI tool's check: installed at `version`, signed in with a subscription. */
export function aiRuntime(
  id: string,
  version: string,
  patch: Partial<AgentRuntimeInfo> = {},
): AgentRuntimeInfo {
  const f = FACTS[id]!;
  return {
    id,
    label: f.label,
    provider: f.provider,
    providerLabel: f.providerLabel,
    installation: { state: "installed", executable: `/bin/${id}`, version, detail: null },
    auth: { state: "subscription", method: `${f.label} subscription`, detail: null },
    capabilities: {
      streamingText: true,
      resume: true,
      cancel: true,
      structuredResults: true,
      billingCheckedPerTurn: id !== "copilot",
      toolPosture: "Conversation only",
      effortLevels: [],
      knownModels: MODELS[id] ?? [],
      runsOtherMakers: OTHER_MAKERS.has(id),
    },
    installHint: `Install ${f.label}.`,
    loginHint: f.signIn ? `Open a terminal, run: ${f.signIn}` : "Add a key on its card.",
    ready: true,
    // Checked long ago (a sign-in tab that ends after it waits for a newer check).
    checkedAt: 1,
    checkedVersion: version,
    account: { signIn: f.signIn, signOut: f.signOut },
    reportedModels: null,
    held: null,
    ...patch,
  };
}

export const idle = (patch: Partial<AiToolUpdate> = {}): AiToolUpdate => ({
  state: "idle",
  from: null,
  to: null,
  tasksUsing: 0,
  message: null,
  oldStillWorks: null,
  automatic: false,
  at: null,
  ...patch,
});

/** The page's part for one AI tool: up to date, nothing going on. */
export function aiTool(id: string, patch: Partial<AiToolState> = {}): AiToolState {
  return {
    runtimeId: id,
    builtIn: PAID.has(id),
    newestFrom:
      id === "grok"
        ? "own"
        : id === "kimi" || id === "antigravity" || PAID.has(id)
          ? "updateChecks"
          : "published",
    newest: null,
    newestCheckedAt: T0,
    newestProblem: null,
    canUpdate: id !== "ollama" && !PAID.has(id),
    update: idle(),
    updateByHand: null,
    outOfService: null,
    reportsPlanLeft: id === "claude-code" || id === "codex" || id === "copilot",
    plan: null,
    payment: PAID.has(id) ? "paidKey" : "subscription",
    paidKey: null,
    paidBlocked: null,
    keyKeptIn: PAID.has(id) ? "Windows Credential Manager" : null,
    hasModelList: id !== "claude-code",
    modelsCheckLeavesATrace: id === "kimi",
    checking: false,
    ...patch,
  };
}

export function aiPage(tools: AiToolState[], patch: Partial<AiToolsPage> = {}): AiToolsPage {
  return { tools, autoUpdate: false, lastLookedAt: T0, looking: false, ...patch };
}

/** The Router's view of an AI tool (its usage limit, new and unlisted models). */
export function route(id: string, patch: Partial<ToolInfo> = {}): ToolInfo {
  const f = FACTS[id]!;
  return routedTool(id, {
    label: f.label,
    company: f.provider,
    companyLabel: f.providerLabel,
    knownModels: MODELS[id] ?? [],
    runsOtherMakers: OTHER_MAKERS.has(id),
    ...patch,
  });
}
