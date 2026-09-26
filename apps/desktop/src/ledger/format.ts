import type {
  HandoffOutcome,
  LedgerEvent,
  SyntheticTaskAction,
  TaskState,
  TurnOutcome,
} from "@plenipo/types";

import { HANDOFF_OUTCOME_LABEL, OUTCOME_LABEL } from "../agents/format";
import { capabilityLabel } from "../guard/format";

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
    case "approval.requested": {
      const request = (
        typeof p.request === "object" && p.request !== null ? p.request : {}
      ) as Record<string, unknown>;
      return `Waiting for your approval: ${str(request.summary) ?? capabilityLabel(str(p.actionType) ?? "")}`;
    }
    case "approval.resolved": {
      const what = str(p.summary) ?? capabilityLabel(str(p.actionType) ?? "");
      const note =
        str(p.note) && !/^(Approved|Refused) by you\.$/.test(str(p.note) ?? "")
          ? ` (${str(p.note)})`
          : "";
      return `${p.state === "approved" ? "Approved" : "Not approved"}: ${what}${note}`;
    }
    case "approval.expired":
      return `Approval expired: ${str(p.summary) ?? capabilityLabel(str(p.actionType) ?? "")}${
        str(p.note) ? ` (${str(p.note)})` : ""
      }`;
    case "artifact.recorded":
      return `Artifact recorded: ${str(p.path) ?? str(p.uri) ?? ""}`;
  }
  const agent = describeAgentEvent(e.eventType, p);
  if (agent !== null) return agent;
  const liaison = describeLiaisonEvent(e.eventType, p);
  if (liaison !== null) return liaison;
  if (e.eventType.startsWith("execution.")) {
    const label = str(p.label) ?? "Program";
    const code = typeof p.exitCode === "number" ? ` · exit ${p.exitCode}` : "";
    return `${label}: ${e.eventType.slice("execution.".length).replace(/_/g, " ")}${code}`;
  }
  const org = describeOrgEvent(e.eventType, p);
  if (org !== null) return org;
  const router = describeRouterEvent(e.eventType, p);
  if (router !== null) return router;
  const guard = describeGuardEvent(e.eventType, p);
  if (guard !== null) return guard;
  if (e.eventType.startsWith("org.")) {
    return `Organization: ${e.eventType.slice(4).replace(/_/g, " ")} ${str(p.name) ?? ""}`.trim();
  }
  return e.eventType;
}

const OVERSIGHT_WORD: Record<string, string> = {
  review: "reviewer",
  qa: "QA evaluator",
  security: "security auditor",
};

/** "Claude Code · opus", from an event's `runtimeId` and `model`. */
function tool(p: Record<string, unknown>): string {
  const id = str(p.runtimeId) ?? "";
  const name = TOOL_NAMES[id] ?? id;
  return str(p.model) ? `${name} · ${str(p.model)}` : name;
}

/** " — <why this model>" when the event carries the Router's explanation. */
function routed(p: Record<string, unknown>): string {
  const routing = p.routing;
  if (typeof routing !== "object" || routing === null) return "";
  const reason = str((routing as Record<string, unknown>).reason);
  return reason ? ` — ${reason}` : "";
}

/** Phase 6: the model settings. */
function describeRouterEvent(type: string, p: Record<string, unknown>): string | null {
  const model = (typeof p.model === "object" && p.model !== null ? p.model : {}) as Record<
    string,
    unknown
  >;
  switch (type) {
    case "router.model_saved":
      return `Model saved: ${str(model.label) ?? "a model"}`;
    case "router.model_removed":
      return `Model removed: ${str(model.label) ?? "a model"}`;
    case "router.models_added":
      return "Each AI tool's default model was added to your models";
    case "router.policy_changed":
      return `Model choices changed for ${str(p.role) ?? "a role"}`;
    case "router.policies_added":
      return "Built-in roles got their starting model choices";
    case "router.options_changed":
      return "Usage-limit setting changed";
    case "router.limit_cleared":
      return `You asked to try ${str(p.label) ?? "an AI tool"} again after its usage limit`;
  }
  return null;
}

/** Phase 7: permissions given, used, blocked, and revoked, and the owner's settings. */
function describeGuardEvent(type: string, p: Record<string, unknown>): string | null {
  const worker = str(p.worker) ?? "A worker";
  switch (type) {
    case "guard.grant_opened": {
      const perms =
        typeof p.permissions === "object" && p.permissions !== null
          ? Object.entries(p.permissions as Record<string, unknown>).map(
              ([c, l]) => `${capabilityLabel(c)}${l === "ask" ? " (asks you)" : ""}`,
            )
          : [];
      return `Permissions given to ${worker}${
        str(p.folder) ? ` in ${str(p.folder)}` : ""
      }: ${perms.join(", ") || "none"}`;
    }
    case "guard.grant_closed":
      return `Permissions ended for ${worker}: ${Number(p.used ?? 0)} done, ${Number(
        p.blocked ?? 0,
      )} blocked, ${Number(p.asked ?? 0)} asked you`;
    case "guard.grant_revoked":
      return `You revoked ${worker}'s permissions`;
    case "guard.grant_skipped":
      return str(p.reason) ?? `${worker} got no tools`;
    case "guard.denied":
      return `Blocked: ${worker} tried to ${str(p.summary) ?? "do something"} — ${brief(p.reason, 240)}`;
    case "capability.used":
      return `${worker}: ${str(p.summary) ?? "used a tool"}${p.ok === false ? " (failed)" : ""}${
        str(p.result) ? ` — ${brief(p.result)}` : ""
      }`;
    case "guard.defaults_added":
      return "Plenipo Guard's starting permission settings were stored";
    case "guard.sets_added":
      return "Built-in permission sets were added back";
    case "guard.roles_seeded":
      return "Built-in roles got their starting permission sets";
    case "guard.set_added":
    case "guard.set_changed": {
      const set = (typeof p.set === "object" && p.set !== null ? p.set : {}) as Record<
        string,
        unknown
      >;
      return `Permission set ${type === "guard.set_added" ? "added" : "changed"}: ${
        str(set.name) ?? ""
      }`;
    }
    case "guard.set_removed":
      return `Permission set removed: ${str(p.name) ?? ""}`;
    case "guard.role_assigned":
      return `${str(p.role) ?? "A role"}'s permissions changed`;
    case "guard.department_limited":
      return `${str(p.department) ?? "A department"}'s permission limit changed`;
    case "guard.commands_changed":
      return "Command lists changed";
    case "guard.files_changed":
      return "Blocked files changed";
    case "guard.sensitive_changed":
      return "A sensitive-action setting changed";
    case "guard.options_changed":
      return "Approval wait changed";
    case "vault.secret_added":
      return `Secret stored: ${str(p.name) ?? ""}`;
    case "vault.secret_changed":
      return `Secret changed: ${str(p.name) ?? ""}`;
    case "vault.secret_removed":
      return `Secret removed: ${str(p.name) ?? ""}`;
  }
  return null;
}

/** Phase 5: the organization's structure, its agents, and the workers it spawns. */
function describeOrgEvent(type: string, p: Record<string, unknown>): string | null {
  const title = str(p.title) ?? "a position";
  const name = str(p.name) ?? "";
  const why = str(p.reason) ? ` (${str(p.reason)})` : "";
  switch (type) {
    case "org.position_created":
      return `Position created: ${title}`;
    case "org.position_updated":
      return str(p.title) ? `Position renamed to ${title}` : "Position's AI tool or model changed";
    case "org.position_moved":
      return `${title} now reports to ${p.to === null ? "you" : "a new lead"}`;
    case "org.position_archived":
      return `Position archived: ${title}${why}`;
    case "org.agent_hired":
      return `Agent hired for ${title}`;
    case "org.agent_retired":
      return `Agent retired from ${title}${why}`;
    case "org.worker_spawned":
      return `Worker brought in for ${title}${routed(p)}`;
    case "org.agent_routed":
      return `${title}'s agent starts its conversation on ${tool(p)}${routed(p)}`;
    case "org.worker_started":
      return "Worker started";
    case "org.worker_retired":
      return p.lifecycle === "failed"
        ? "Worker failed and left the organization"
        : "Worker finished and left the organization";
    case "org.oversight_assigned":
      return `${str(p.overseer) ?? "A position"} is now the ${
        OVERSIGHT_WORD[str(p.kind) ?? ""] ?? "overseer"
      } for ${str(p.target) ?? "a"}'s team`;
    case "org.oversight_ended":
      return `Oversight assignment ended${why}`;
    case "org.department_created":
      return `Department created: ${name}`;
    case "org.department_updated":
      return `Department updated: ${name}`;
    case "org.department_deleted":
      return `Department removed: ${name}`;
    case "org.project_created":
      return `Project created: ${name}`;
    case "org.project_updated":
      return `Project settings updated: ${name}`;
    case "org.project_archived":
      return `Project archived with its team: ${name}`;
    case "org.project_reassigned":
      return `Project moved to another department: ${name}`;
    case "org.role_created":
      return `Role added: ${name}`;
    case "org.settings_changed":
      return "Organization settings changed";
  }
  return null;
}

const TOOL_NAMES: Record<string, string> = { "claude-code": "Claude Code", codex: "Codex" };

/**
 * Who recorded an event or asked for a task, in plain words: Plenipo's own parts are "Plenipo",
 * the owner is "you", and an agent is named by its AI tool (`agent:codex` → "Codex").
 */
export function sourceLabel(source: string, { capitalize = false } = {}): string {
  if (source === "owner") return capitalize ? "You" : "you";
  if (source === "runtime" || source === "plenipo" || source === "core") return "Plenipo";
  if (source === "liaison") return "Liaison";
  if (source === "guard") return "Guard";
  if (source.startsWith("agent:")) {
    const id = source.slice("agent:".length);
    return TOOL_NAMES[id] ?? id;
  }
  return source;
}

/** First line of agent-provided text, kept short for the trail. */
function brief(v: unknown, max = 160): string {
  const line = (str(v) ?? "").split("\n").find((l) => l.trim() !== "") ?? "";
  return line.length > max ? `${line.slice(0, max - 1)}…` : line;
}

/** Phase 3: agent turn activity and worker conversation events. */
function describeAgentEvent(type: string, p: Record<string, unknown>): string | null {
  switch (type) {
    case "agent.session_bound":
      return `Conversation ${str(p.providerSessionId) ?? "started"}${
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
      return `Worker conversation opened on ${str(p.runtime) ?? "an AI tool"}`;
    case "session.bound":
      return `Worker conversation linked to provider session ${str(p.providerSessionId) ?? "?"}`;
    case "session.closed":
      return "Worker conversation closed";
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
