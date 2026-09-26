import type {
  AgentEvent,
  AgentRuntimeInfo,
  AuthState,
  HandoffOutcome,
  HandoffState,
  InstallState,
  TokenUsage,
  TurnOutcome,
} from "@plenipo/types";

export const OUTCOME_LABEL: Record<TurnOutcome, string> = {
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
  timedOut: "Timed out",
  usageLimited: "Usage limit reached",
  authRequired: "Sign-in required",
  billingNotAllowed: "Blocked: API billing",
  providerUnavailable: "Provider unavailable",
  malformedOutput: "Unreadable output",
  crashed: "Crashed",
  interrupted: "Interrupted",
};

/** Badge style for an outcome (reuses the task badge palette). */
export function outcomeTone(
  outcome: TurnOutcome,
): "succeeded" | "failed" | "blocked" | "cancelled" {
  switch (outcome) {
    case "completed":
      return "succeeded";
    case "cancelled":
    case "interrupted":
      return "cancelled";
    case "usageLimited":
    case "authRequired":
    case "billingNotAllowed":
      return "blocked";
    default:
      return "failed";
  }
}

export const HANDOFF_OUTCOME_LABEL: Record<HandoffOutcome, string> = {
  ...OUTCOME_LABEL,
  rejected: "Refused by Liaison",
};

export function handoffOutcomeTone(
  outcome: HandoffOutcome,
): "succeeded" | "failed" | "blocked" | "cancelled" {
  return outcome === "rejected" ? "blocked" : outcomeTone(outcome);
}

export const HANDOFF_STATE_LABEL: Record<HandoffState, string> = {
  accepted: "Waiting for a worker",
  dispatched: "Worker running",
  answered: "Answered",
  cancelled: "Cancelled",
  rejected: "Refused",
};

/** Badge style for a handoff's state (reuses the task badge palette). */
export function handoffStateTone(
  state: HandoffState,
): "queued" | "running" | "succeeded" | "blocked" | "cancelled" {
  switch (state) {
    case "accepted":
      return "queued";
    case "dispatched":
      return "running";
    case "answered":
      return "succeeded";
    case "cancelled":
      return "cancelled";
    case "rejected":
      return "blocked";
  }
}

export const INSTALL_LABEL: Record<InstallState, string> = {
  checking: "Checking…",
  installed: "Installed",
  notInstalled: "Not installed",
  unsupported: "Unsupported install",
  broken: "Not working",
};

export const AUTH_LABEL: Record<AuthState, string> = {
  checking: "Checking…",
  subscription: "Signed in (subscription)",
  unverified: "Signed in (billing unverified)",
  apiKey: "API key — not allowed",
  thirdPartyCloud: "Third-party cloud — not allowed",
  signedOut: "Not signed in",
  unknown: "Sign-in unknown",
};

export function runtimeStatus(r: AgentRuntimeInfo): {
  text: string;
  tone: "ok" | "warn" | "bad" | "muted";
} {
  if (r.installation.state === "checking") return { text: "Checking…", tone: "muted" };
  if (r.ready) return { text: "Ready", tone: "ok" };
  if (r.installation.state !== "installed")
    return { text: INSTALL_LABEL[r.installation.state], tone: "bad" };
  return { text: AUTH_LABEL[r.auth.state], tone: "warn" };
}

/** Why an AI tool cannot take work, with what to do about it. `null` when ready. */
export function notReadyHint(r: AgentRuntimeInfo): string | null {
  if (r.ready) return null;
  if (r.installation.state === "checking") return "Still checking this AI tool…";
  if (r.installation.state !== "installed") {
    return [r.installation.detail, r.installHint].filter(Boolean).join(" ");
  }
  return [r.auth.detail, r.loginHint].filter(Boolean).join(" ");
}

export function describeUsage(u: TokenUsage): string {
  const cached = u.cachedInputTokens > 0 ? ` (${u.cachedInputTokens.toLocaleString()} cached)` : "";
  return `${u.inputTokens.toLocaleString()} in${cached} · ${u.outputTokens.toLocaleString()} out`;
}

/** Short label + text for one activity event. */
export function describeActivity(e: AgentEvent): {
  label: string;
  text: string;
  tone?: "warn" | "bad" | undefined;
} {
  switch (e.type) {
    case "sessionStarted":
      return {
        label: "Session",
        text:
          [e.model && `model ${e.model}`, e.providerSessionId && `session ${e.providerSessionId}`]
            .filter(Boolean)
            .join(" · ") || "started",
      };
    case "textDelta":
    case "message":
      return { label: "Agent", text: e.text };
    case "reasoning":
      return { label: "Thinking", text: e.text };
    case "toolUse":
      return { label: e.tool, text: e.summary };
    case "toolResult":
      return {
        label: e.tool ?? "Tool result",
        text: e.summary || (e.isError ? "failed" : "done"),
        tone: e.isError ? "bad" : undefined,
      };
    case "notice":
      return {
        label: e.level === "warning" ? "Warning" : "Note",
        text: e.text,
        tone: e.level === "warning" ? "warn" : undefined,
      };
    case "usage":
      return { label: "Usage", text: describeUsage(e.usage) };
  }
}
