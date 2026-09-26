// Shared agent DTO fixtures for tests.
import type {
  AgentActivity,
  AgentEvent,
  AgentRuntimeInfo,
  AgentSession,
  AgentTurn,
} from "@plenipo/types";

export const runtime = (id: string, ready = true): AgentRuntimeInfo => ({
  id,
  label: id === "codex" ? "Codex" : "Claude Code",
  provider: id === "codex" ? "openai" : "anthropic",
  providerLabel: id === "codex" ? "OpenAI" : "Anthropic",
  installation: { state: "installed", executable: `/bin/${id}`, version: "1.0.0", detail: null },
  auth: ready
    ? { state: "subscription", method: "Subscription", detail: null }
    : { state: "signedOut", method: null, detail: null },
  capabilities: {
    streamingText: id !== "codex",
    resume: true,
    cancel: true,
    structuredResults: true,
    billingCheckedPerTurn: id !== "codex",
    toolPosture: "Conversation only",
  },
  installHint: "Install it.",
  loginHint: "Run the login command.",
  ready,
  checkedAt: 1,
});

export const session = (id: string, patch: Partial<AgentSession> = {}): AgentSession => ({
  id,
  runtimeId: "claude-code",
  provider: "anthropic",
  providerSessionId: null,
  providerSessionConfirmed: false,
  model: null,
  title: `Session ${id}`,
  state: "open",
  workingDir: `/w/${id}`,
  createdAt: 1,
  updatedAt: 1,
  turnCount: 1,
  activeTaskId: null,
  ...patch,
});

export const turn = (taskId: string, patch: Partial<AgentTurn> = {}): AgentTurn => ({
  taskId,
  sessionId: "s1",
  number: 1,
  objective: "Say hello",
  executionId: "e1",
  running: true,
  result: null,
  startedAt: 1,
  endedAt: null,
  ...patch,
});

export const activity = (taskId: string, seq: number, event: AgentEvent): AgentActivity => ({
  sessionId: "s1",
  taskId,
  seq,
  ts: seq,
  event,
});
