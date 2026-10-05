import type {
  AgentRuntimeInfo,
  AgentTurn,
  AuthState,
  HandoffOutcome,
  HandoffState,
  InstallState,
  TokenUsage,
  TurnOutcome,
  TurnResult,
} from "@plenipo/types";

export const OUTCOME_LABEL: Record<TurnOutcome, string> = {
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
  timedOut: "Timed out",
  usageLimited: "Usage limit reached",
  authRequired: "Sign-in required",
  billingNotAllowed: "Blocked: API billing",
  providerUnavailable: "AI tool unavailable",
  malformedOutput: "Unreadable output",
  crashed: "AI tool stopped unexpectedly",
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
  apiKey: "Own API key outside Plenipo — not used",
  thirdPartyCloud: "Third-party cloud — not allowed",
  signedOut: "Not signed in",
  unknown: "Sign-in unknown",
  paidKey: "Key saved (paid per use)",
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

/**
 * Why a new task on an AI tool waits for now (Phase 19): it is being updated (ADR-059 §4), or
 * its sign-in tab is open (ADR-058 §5). `null` when nothing holds it.
 */
export function heldNote(r: AgentRuntimeInfo): string | null {
  switch (r.held) {
    case "update":
      return `Waiting: ${r.label} is being updated. A new task starts on it when that's done.`;
    case "signIn":
      return `${r.label}'s sign-in tab is open. A new task waits until it closes, or 10 minutes at most.`;
    case "stopAll":
      return "All work is stopped. A new task starts when you press Allow again.";
    default:
      return null;
  }
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

/** The size of what Plenipo sent with one step of a task (ADR-044). */
export type PromptSize = NonNullable<TurnResult["prompt"]>;

/** What a task's first message carried with it, in plain words. */
const PROMPT_KIND: Record<PromptSize["brief"], string | null> = {
  full: "full instructions",
  reminder: "a short reminder",
  replies: "replies",
  plain: null,
};

/**
 * A task's tokens, added up over its steps like its sizes (`describePrompt`): the final
 * result's alone for older tasks, or when no step reported any.
 */
export function turnUsage(turn: AgentTurn): TokenUsage | null {
  const reported = turn.steps.flatMap((s) => (s.result?.usage ? [s.result.usage] : []));
  if (reported.length === 0) return turn.result?.usage ?? null;
  return reported.reduce((sum, u) => ({
    inputTokens: sum.inputTokens + u.inputTokens,
    cachedInputTokens: sum.cachedInputTokens + u.cachedInputTokens,
    outputTokens: sum.outputTokens + u.outputTokens,
  }));
}

/** The sizes recorded for a task's steps, in order (the final result's alone for older tasks). */
export function turnPromptSizes(turn: AgentTurn): PromptSize[] {
  const sizes = turn.steps.flatMap((s) => (s.result?.prompt ? [s.result.prompt] : []));
  if (sizes.length > 0) return sizes;
  return turn.result?.prompt ? [turn.result.prompt] : [];
}

/**
 * How much of Plenipo's own text went with a task, shown beside its token counts (ADR-044):
 * "Plenipo's own text: 0.4 KB (a short reminder)". The steps' sizes are added up; `null` when
 * nothing was recorded or Plenipo added nothing of its own.
 */
export function describePrompt(sizes: PromptSize[]): string | null {
  const first = sizes[0];
  const own = sizes.reduce((sum, s) => sum + s.ownBytes, 0);
  if (!first || own === 0) return null;
  const size = `${Math.max(0.1, own / 1024).toFixed(1)} KB`;
  const kinds = [PROMPT_KIND[first.brief], sizes.length > 1 ? "then replies" : null].filter(
    (k): k is string => k !== null,
  );
  return kinds.length > 0
    ? `Plenipo's own text: ${size} (${kinds.join(", ")})`
    : `Plenipo's own text: ${size}`;
}
