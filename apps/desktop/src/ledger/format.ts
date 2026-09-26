import type {
  HandoffOutcome,
  LedgerEvent,
  SyntheticTaskAction,
  TaskState,
  TurnOutcome,
} from "@plenipo/types";

import { HANDOFF_OUTCOME_LABEL, OUTCOME_LABEL } from "../agents/format";

export const TASK_STATE_LABEL: Record<TaskState, string> = {
  queued: "Queued",
  running: "Running",
  blocked: "Blocked",
  awaitingApproval: "Awaiting approval",
  succeeded: "Succeeded",
  failed: "Failed",
  cancelled: "Cancelled",
};

export const ACTION_LABEL: Record<SyntheticTaskAction, string> = {
  start: "Start",
  block: "Block",
  awaitApproval: "Await approval",
  resume: "Resume",
  complete: "Complete",
  fail: "Fail",
  cancel: "Cancel",
  addChild: "Add step",
};

/** Actions the ledger will accept from each state (mirrors the Rust state machine). */
export const ACTIONS_FOR: Record<TaskState, SyntheticTaskAction[]> = {
  queued: ["start", "fail", "cancel", "addChild"],
  running: ["block", "awaitApproval", "complete", "fail", "cancel", "addChild"],
  blocked: ["resume", "fail", "cancel", "addChild"],
  awaitingApproval: ["resume", "fail", "cancel", "addChild"],
  succeeded: [],
  failed: [],
  cancelled: [],
};

const str = (v: unknown): string | null => (typeof v === "string" ? v : null);

function stateLabel(v: unknown): string {
  const s = str(v);
  return s && s in TASK_STATE_LABEL ? TASK_STATE_LABEL[s as TaskState] : (s ?? "?");
}

/** One-line, human-readable description of a ledger event. */
export function describeEvent(e: LedgerEvent): string {
  const p = (e.payload ?? {}) as Record<string, unknown>;
  const reason = str(p.reason) ? ` (${str(p.reason)})` : "";
  switch (e.eventType) {
    case "task.created":
      return `Task created: ${str(p.objective) ?? ""}`.trim();
    case "task.state_changed":
      return `${stateLabel(p.from)} → ${stateLabel(p.to)}${reason}`;
    case "task.transition_rejected":
      return `Rejected: ${stateLabel(p.from)} → ${stateLabel(p.to)} is not allowed${reason}`;
    case "task.child_created":
      return `Sub-task created: ${str(p.objective) ?? ""}`;
    case "task.assigned":
      return `Assigned to ${str(p.to) ?? "nobody"}`;
    case "approval.requested":
      return `Approval requested: ${str(p.actionType) ?? ""}`;
    case "approval.resolved":
    case "approval.expired":
      return `Approval ${str(p.state) ?? ""}: ${str(p.actionType) ?? ""}`;
    case "artifact.recorded":
      return `Artifact recorded: ${str(p.path) ?? str(p.uri) ?? ""}`;
  }
  const agent = describeAgentEvent(e.eventType, p);
  if (agent !== null) return agent;
  const liaison = describeLiaisonEvent(e.eventType, p);
  if (liaison !== null) return liaison;
  if (e.eventType.startsWith("execution.")) {
    const label = str(p.label) ?? "Process";
    const code = typeof p.exitCode === "number" ? ` · exit ${p.exitCode}` : "";
    return `${label}: ${e.eventType.slice("execution.".length).replace(/_/g, " ")}${code}`;
  }
  if (e.eventType.startsWith("org.")) {
    return `Organization: ${e.eventType.slice(4).replace(/_/g, " ")} ${str(p.name) ?? ""}`.trim();
  }
  return e.eventType;
}

/** First line of agent-provided text, kept short for the trail. */
function brief(v: unknown, max = 160): string {
  const line = (str(v) ?? "").split("\n").find((l) => l.trim() !== "") ?? "";
  return line.length > max ? `${line.slice(0, max - 1)}…` : line;
}

/** Phase 3: agent turn activity and runtime session events. */
function describeAgentEvent(type: string, p: Record<string, unknown>): string | null {
  switch (type) {
    case "agent.session_bound":
      return `Provider session ${str(p.providerSessionId) ?? "started"}${
        str(p.model) ? ` · model ${str(p.model)}` : ""
      }`;
    case "agent.message":
      return `Agent: ${brief(p.text)}`;
    case "agent.tool_use":
      return `Tool ${str(p.tool) ?? ""}: ${brief(p.summary)}`.trim();
    case "agent.tool_result":
      return `Tool result${p.isError === true ? " (error)" : ""}: ${brief(p.summary)}`;
    case "agent.notice":
      return `Notice: ${brief(p.text)}`;
    case "agent.result": {
      const outcome = str(p.outcome);
      const label =
        outcome && outcome in OUTCOME_LABEL ? OUTCOME_LABEL[outcome as TurnOutcome] : outcome;
      return `Result: ${label ?? "?"} — ${brief(p.summary)}`;
    }
    case "session.opened":
      return `Worker session opened on ${str(p.runtime) ?? "a runtime"}`;
    case "session.bound":
      return `Worker session bound to provider session ${str(p.providerSessionId) ?? "?"}`;
    case "session.closed":
      return "Worker session closed";
  }
  return null;
}

/** `runtime:codex` → `codex`; other addresses as written. */
function address(v: unknown): string {
  const a = str(v) ?? "?";
  return a.startsWith("runtime:") ? a.slice("runtime:".length) : a;
}

function handoffOutcome(v: unknown): string {
  const o = str(v);
  return o && o in HANDOFF_OUTCOME_LABEL ? HANDOFF_OUTCOME_LABEL[o as HandoffOutcome] : (o ?? "?");
}

function count(v: unknown): number {
  return Array.isArray(v) ? v.length : 0;
}

/** Phase 4: handoffs between workers through Plenipo Liaison. */
function describeLiaisonEvent(type: string, p: Record<string, unknown>): string | null {
  const why = str(p.reason) ? `: ${brief(p.reason, 200)}` : "";
  switch (type) {
    case "liaison.handoff_requested":
      return `Handoff requested → ${address(p.destination)}: ${brief(p.objective)}`;
    case "liaison.handoff_received":
      return `Received as a handoff${
        typeof p.depth === "number" ? ` (depth ${p.depth})` : ""
      }: ${brief(p.objective)}`;
    case "liaison.handoff_rejected":
      return `Handoff refused${why}`;
    case "liaison.duplicate_ignored":
      return "Duplicate handoff request ignored";
    case "liaison.dispatched":
      return "Handoff worker started";
    case "liaison.dispatch_failed":
      return `Handoff worker could not start${why}`;
    case "liaison.reply_sent":
      return `Reply sent: ${handoffOutcome(p.outcome)} — ${brief(p.summary)}`;
    case "liaison.reply_received":
      return `Reply received: ${handoffOutcome(p.outcome)} — ${brief(p.summary)}`;
    case "liaison.replies_delivered": {
      const n = count(p.messageIds);
      return `Continued with ${n} handoff repl${n === 1 ? "y" : "ies"}`;
    }
    case "liaison.reply_discarded":
      return `Handoff repl${count(p.messageIds) === 1 ? "y" : "ies"} discarded${why}`;
    case "liaison.handoff_cancelled":
      return `Handoff cancelled${why}`;
    case "liaison.reply_refused":
      return `Reply refused${why}`;
    case "liaison.sender_rejected":
      return `Handoff requests ignored${why}`;
    case "liaison.delivery_failed":
      return `Handoff replies could not be delivered${why}`;
  }
  return null;
}

export function isRejection(e: LedgerEvent): boolean {
  return e.eventType === "task.transition_rejected";
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}
