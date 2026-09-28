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
  awaitingApproval: "Waiting for you",
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

/** The owner's browser choices (ADR-028), in plain words. */
const BROWSER_CHOICES: Record<string, string> = {
  automatic: "Automatic (Microsoft Edge, or Google Chrome without it)",
  edge: "Microsoft Edge",
  chrome: "Google Chrome",
};

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
  const control = describeControlEvent(e.eventType, p);
  if (control !== null) return control;
  const server = describeServerEvent(e.eventType, p);
  if (server !== null) return server;
  const terminal = describeTerminalEvent(e.eventType, p);
  if (terminal !== null) return terminal;
  const learned = describeLearningEvent(e.eventType, p);
  if (learned !== null) return learned;
  const upkeep = describeUpkeepEvent(e.eventType, p);
  if (upkeep !== null) return upkeep;
  if (e.eventType.startsWith("org.")) {
    return `Organization: ${e.eventType.slice(4).replace(/_/g, " ")} ${str(p.name) ?? ""}`.trim();
  }
  return e.eventType;
}

const RECOVERY_WORDS: Record<string, string> = {
  crash: "Plenipo closed unexpectedly",
  windowsRestart: "Windows closed Plenipo (a restart, a shutdown, or signing out)",
  layoutChange: "Plenipo was stopped while updating the Ledger's layout",
};

const BACKUP_WORDS: Record<string, string> = {
  manual: "made by you",
  daily: "the daily one",
  beforeUpgrade: "before a new version",
  beforeUpdate: "before an update",
  beforeMigration: "before a layout change",
  beforeRestore: "before a restore",
};

/** Phase 13: recovery, backups and restore, versions and updates. */
function describeUpkeepEvent(type: string, p: Record<string, unknown>): string | null {
  switch (type) {
    case "plenipo.recovered": {
      const stopped = Array.isArray(p.stoppedTasks) ? p.stoppedTasks.length : 0;
      const what = RECOVERY_WORDS[str(p.cause) ?? ""] ?? "Plenipo did not close normally";
      return `${what}; ${stopped === 1 ? "1 task was" : `${stopped} tasks were`} stopped`;
    }
    case "plenipo.run_again":
      return "You ran this task again";
    case "plenipo.window_recovered":
      return p.reopened === true
        ? "The window stopped responding and was opened again"
        : "The window stopped responding and was reloaded";
    case "plenipo.version_changed":
      return str(p.from)
        ? `Plenipo was updated from ${str(p.from)} to ${str(p.to) ?? "a new version"}`
        : `Plenipo ${str(p.to) ?? ""} started for the first time on this Ledger`.replace("  ", " ");
    case "plenipo.update_available":
      return `Plenipo ${str(p.version) ?? ""} is ready to install`;
    case "plenipo.update_installing":
      return `Installing Plenipo ${str(p.to) ?? ""} (the Ledger was backed up first)`;
    case "plenipo.update_failed":
      return `The update was not installed: ${str(p.message) ?? ""}`.trim();
    case "ledger.backed_up":
      return `The Ledger was backed up (${BACKUP_WORDS[str(p.kind) ?? ""] ?? "a backup"})`;
    case "ledger.restore_requested":
      return `You chose to restore the Ledger from ${str(p.backup) ?? "a backup"}`;
    case "ledger.restored":
      return `The Ledger was restored from ${str(p.backup) ?? "a backup"}`;
    case "ledger.restore_failed":
      return "The Ledger could not be restored; it was left as it was";
    case "guard.request_refused":
      return `Plenipo refused to reach ${str(p.host) ?? "an address"} for ${str(p.purpose) ?? "itself"}`;
    case "guard.settings_reset":
      return "Your permission settings were reset to their starting values (after a backup)";
    case "routing.settings_reset":
      return "Your AI model settings were reset to their starting values (after a backup)";
  }
  return null;
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
    case "router.rule_changed":
      return p.layer === "organization"
        ? "The organization's model and effort rule changed"
        : p.layer === "department"
          ? `The ${str(p.name) ?? "a"} department's model and effort rule changed`
          : `${str(p.name) ?? "An agent"}'s own model and effort rule changed`;
    case "router.rules_forgotten":
      return "Rules of things deleted for good were removed";
  }
  return null;
}

/** " with Website, Shop" — names in a list, or nothing. */
function names(list: unknown, lead: string): string {
  const all = Array.isArray(list) ? list.map(String).filter((x) => x !== "") : [];
  return all.length > 0 ? `${lead}${all.join(", ")}` : "";
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
    case "tool_server.ticket_refused": {
      // ADR-034: a program outside the AI tool's own process tree presented the ticket.
      const program = (v: unknown) => (typeof v === "number" ? String(v) : "unknown");
      return `Blocked: a program outside ${worker}'s AI tool tried to use ${worker}'s tools (program ${program(
        p.connectingPid,
      )}; the AI tool is program ${program(p.expectedRootPid)})`;
    }
    case "tool_server.ticket_unchecked":
      return `Plenipo could not check which program connected to ${worker}'s tools on this computer, so it let it through`;
    case "guard.approvals_limited":
      // B6: the worker asked again while its earlier requests still wait, or asked too often;
      // Plenipo refused that call and made no card.
      return p.limit === "minute"
        ? `Blocked: ${worker} asked for your approval too many times in one minute`
        : `Blocked: ${worker} asked for your approval again while ${Number(
            p.waiting ?? 0,
          )} of its requests were waiting for you`;
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
    case "guard.sets_updated":
      return "Built-in permission sets you had not changed were brought up to date";
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
    case "guard.websites_changed":
      return "Website lists changed";
    case "guard.switches_changed":
      return "Switches changed (Settings → Switches)";
    case "guard.browser_chosen":
      return `Plenipo's browser set to ${BROWSER_CHOICES[str(p.browserChoice) ?? ""] ?? "another browser"}`;
    case "guard.websites_added":
      return "Plenipo's starting website lists were stored";
    case "vault.secret_added":
      return `Secret stored: ${str(p.name) ?? ""}`;
    case "vault.secret_changed":
      return `Secret changed: ${str(p.name) ?? ""}`;
    case "vault.secret_removed":
      return `Secret removed: ${str(p.name) ?? ""}`;
  }
  return null;
}

/** ADR-024: lessons workers learn from their work. */
function describeLearningEvent(type: string, p: Record<string, unknown>): string | null {
  const worker = str(p.worker) ?? "A worker";
  const text = brief(p.text, 200);
  switch (type) {
    case "lesson.added":
      return p.state === "kept"
        ? `${worker} learned (kept on its own): ${text}`
        : `${worker} learned something (waiting for you): ${text}`;
    case "lesson.kept":
      return `You kept a lesson: ${text}`;
    case "lesson.discarded":
      return `You discarded a lesson: ${text}`;
    case "lesson.removed":
      return `You removed a lesson: ${text}`;
    case "learning.switched":
      return p.enabled === false
        ? "You switched worker learning off"
        : "You switched worker learning on";
    case "learning.role_changed":
      return p.auto === true
        ? `${str(p.name) ?? "A role"} now learns on its own`
        : `${str(p.name) ?? "A role"}'s lessons now wait for you`;
    case "learning.role_switched":
      return p.learns === false
        ? `${str(p.name) ?? "A role"}'s agents stop learning`
        : `${str(p.name) ?? "A role"}'s agents learn again`;
    case "learning.agent_switched":
      return p.learns === null || p.learns === undefined
        ? `${str(p.title) ?? "An agent"} learns like its role`
        : p.learns === false
          ? `${str(p.title) ?? "An agent"} stops learning`
          : `${str(p.title) ?? "An agent"} always learns`;
    case "learning.agents_forgotten":
      return "Learning settings of agents deleted for good were removed";
  }
  return null;
}

const CONTROL_WHAT: Record<string, string> = {
  browser: "Plenipo's browser",
  desktop: "the mouse and keyboard",
  server: "servers",
};

/** Phase 10: Plenipo's browser, the mouse and keyboard, and the owner's Stop and Take over (and,
 * Phase 11, servers). */
function describeControlEvent(type: string, p: Record<string, unknown>): string | null {
  const worker = str(p.worker) ?? "A worker";
  const what = CONTROL_WHAT[str(p.kind) ?? ""] ?? "the browser or the desktop";
  const why = str(p.why) ? ` (${brief(p.why, 200)})` : "";
  switch (type) {
    case "browser.started":
      return `Plenipo's browser ${p.restarted === true ? "restarted" : "started"}${
        str(p.browser) ? ` (${str(p.browser)})` : ""
      }`;
    case "browser.tab_lost":
      return `${worker}'s browser tab closed${why}`;
    case "browser.tab_stopped":
      return `Plenipo stopped ${worker}'s use of the browser${why}`;
    case "browser.opened_by_owner":
      return `You opened Plenipo's browser${str(p.url) ? ` at ${str(p.url)}` : ""}`;
    case "control.started":
      return `${worker} started using ${what}${str(p.reason) ? ` — ${brief(p.reason, 200)}` : ""}`;
    case "control.ended":
      return `${worker} stopped using ${what}${why}`;
    case "control.taken_over":
      return p.kind === "server"
        ? `You disconnected ${worker} from its servers${why}`
        : `You took over ${what} from ${worker}${why}`;
    case "control.stopped": {
      const n = count(p.sessions);
      return n === 0
        ? "You pressed Stop: browser, desktop, and server work is stopped"
        : `You pressed Stop: ${n} worker${n === 1 ? "" : "s"} stopped using the browser, the desktop, or servers`;
    }
    case "control.allowed":
      return "You allowed browser, desktop, and server work again";
    case "control.switched_off": {
      const n = count(p.sessions);
      const off = p.kind === "server" ? "remote computers (SSH)" : what;
      return `You switched ${off} off: ${n} worker${n === 1 ? "" : "s"} stopped`;
    }
  }
  return null;
}

const ENDING_WORDS: Record<string, string> = {
  signal: "was ended by a signal on the server",
  timedOut: "was stopped at its time limit",
  connectionLost: "lost its connection while it ran (whether it finished is unknown)",
  unknown: "ended without the server saying how",
};

/** Phase 11: servers — connections, server IDs, commands and their output, and settings. */
function describeServerEvent(type: string, p: Record<string, unknown>): string | null {
  const worker = str(p.worker) ?? "A worker";
  const server = str(p.server) ?? str(p.name) ?? "a server";
  const production = p.environment === "production" ? " (PRODUCTION)" : "";
  switch (type) {
    case "ssh.connected":
      return `${worker} connected to ${server}${production}${
        str(p.address) ? ` as ${str(p.address)}` : ""
      }`;
    case "ssh.connect_failed":
      return `${worker} could not connect to ${server}: ${brief(p.reason, 200)}`;
    case "ssh.host_key_changed":
      return `Blocked: ${server}'s server ID changed — it showed ${str(p.seen) ?? "another ID"}, not the pinned ${
        str(p.expected) ?? "one"
      }. Nothing was sent to sign in.`;
    case "ssh.command_started":
      return `${worker} ran on ${server}${production}: ${str(p.command) ?? "a command"}${
        str(p.cwd) ? ` (in ${str(p.cwd)})` : ""
      }`;
    case "ssh.output": {
      const n = count(p.lines);
      return `${p.stream === "err" ? "Error output" : "Output"} from ${server} (${n} line${
        n === 1 ? "" : "s"
      })`;
    }
    case "ssh.command_finished": {
      const ending = str(p.ending) ?? "";
      const code = typeof p.exitCode === "number" ? p.exitCode : null;
      const how =
        ending === "exited"
          ? code === 0
            ? "finished"
            : `ended with exit code ${code ?? "?"}`
          : ending === "stopped"
            ? `was stopped${str(p.why) ? ` (${str(p.why)})` : ""}`
            : ending === "refused"
              ? `was not run${str(p.why) ? ` (${str(p.why)})` : ""}`
              : (ENDING_WORDS[ending] ?? "ended");
      const secs =
        typeof p.seconds !== "number"
          ? ""
          : p.seconds < 1
            ? " in under a second"
            : ` after ${p.seconds} s`;
      return `The command on ${server} ${how}${secs}`;
    }
    case "ssh.disconnected":
      return `${worker} disconnected from ${server}${str(p.why) ? ` (${str(p.why)})` : ""}`;
    case "ssh.command_stop_requested":
      return `You pressed Stop on ${worker}'s command`;
    case "ssh.forward_opened":
      return `${worker} forwarded ${str(p.local) ?? "a local port"} to ${str(p.to) ?? "a port"} through ${server}${
        str(p.reason) ? ` — ${brief(p.reason, 200)}` : ""
      }`;
    case "ssh.forward_closed":
      return `Port forward to ${str(p.to) ?? "a port"} through ${server} closed`;
    case "ssh.identity_checked":
      return `You checked the server ID of ${str(p.host) ?? "a server"}${
        typeof p.port === "number" ? `:${p.port}` : ""
      }: ${str(p.fingerprint) ?? ""}`;
    case "ssh.tested":
      return p.ok === true
        ? `You tested ${server}: connected and signed in`
        : `You tested ${server}: ${brief(p.message, 200)}`;
    case "guard.server_added":
      return `Server added: ${server}${production}`;
    case "guard.server_changed":
      return p.pinned === true
        ? `Server changed: ${server} — you pinned its server ID ${str(p.hostKey) ?? ""}`.trim()
        : `Server changed: ${server}`;
    case "guard.server_removed":
      return `Server removed: ${server}`;
    case "vault.server_sign_in_stored": {
      const what = Array.isArray(p.stored) ? (p.stored as unknown[]).map(String).join(", ") : "";
      return `Sign-in stored for ${server}${what ? `: ${what}` : ""} (the value is never shown)`;
    }
  }
  return null;
}

/** "5 minutes", "1 hour 5 minutes", "12 seconds": how long something lasted. */
export function lasted(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  if (s < 60) return `${s} second${s === 1 ? "" : "s"}`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} minute${m === 1 ? "" : "s"}`;
  const h = Math.floor(m / 60);
  const rest = m % 60;
  return `${h} hour${h === 1 ? "" : "s"}${rest ? ` ${rest} minute${rest === 1 ? "" : "s"}` : ""}`;
}

/**
 * Phase 12: the owner's terminal (ADR-031). Only that one opened and closed is recorded, never
 * what was typed or shown.
 */
function describeTerminalEvent(type: string, p: Record<string, unknown>): string | null {
  const where = p.place === "thisPc" ? "this PC" : (str(p.title) ?? "a server");
  const production = p.environment === "production" ? " (PRODUCTION)" : "";
  switch (type) {
    case "terminal.opened":
      return `You opened a terminal on ${where}${production}${
        p.place === "thisPc" && str(p.shell) ? ` (${str(p.shell)})` : ""
      }`;
    case "terminal.closed": {
      const how = typeof p.seconds === "number" ? ` after ${lasted(p.seconds)}` : "";
      return `The terminal on ${where}${production} closed${how}${
        str(p.why) ? `: ${str(p.why)}` : ""
      }`;
    }
  }
  return null;
}

/** A server's output lines in an event, to show under it (Phase 11). */
export function eventOutput(e: LedgerEvent): { lines: string[]; error: boolean } | null {
  if (e.eventType !== "ssh.output") return null;
  const p = (e.payload ?? {}) as Record<string, unknown>;
  const lines = Array.isArray(p.lines) ? (p.lines as unknown[]).map(String) : [];
  return { lines, error: p.stream === "err" };
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
      return p.forGood === true
        ? `Department deleted for good: ${name}${names(p.projects, " with ")}`
        : `Department removed: ${name}`;
    case "org.department_archived":
      return `Department archived with everything in it: ${name}`;
    case "org.department_restored":
      return `Department brought back: ${name}`;
    case "org.project_restored":
      return `Project brought back with its team: ${name}`;
    case "org.project_deleted":
      return `Project deleted for good: ${name}`;
    case "org.position_restored":
      return `Brought back: ${title}`;
    case "org.position_deleted":
      return `Deleted for good: ${title}${str(p.role) ? ` (${str(p.role)})` : ""}`;
    case "org.oversight_not_restored":
      return "A reviewer, QA, or security assignment did not come back (the other side is not on the chart)";
    case "org.agent_saved":
      return `Saved to your Workforce: ${title}`;
    case "org.agent_hired_from_workforce":
      return `Hired from your Workforce: ${title}`;
    case "org.saved_agent_deleted":
      return `Deleted for good from your Workforce: ${title}`;
    case "org.specialty_created":
      return `Specialty added${str(p.role) ? ` to ${str(p.role)}` : ""}: ${name}`;
    case "org.specialty_updated":
      return `Specialty changed: ${name}`;
    case "org.specialty_removed":
      return `Specialty removed: ${name}`;
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
    case "org.role_renamed":
      return `Built-in role renamed to ${name}`;
    case "org.role_updated":
      if (p.template === true) return `Built-in role's instructions updated: ${name}`;
      if (str(p.formerly) && str(p.formerly) !== name) {
        return `Role renamed from ${str(p.formerly)} to ${name}`;
      }
      return name ? `Role changed: ${name}` : "A role's model choices or permissions changed";
    case "org.settings_changed":
      return "Organization settings changed";
  }
  return null;
}

const TOOL_NAMES: Record<string, string> = {
  "claude-code": "Claude Code",
  codex: "Codex",
  grok: "Grok",
  kimi: "Kimi",
  ollama: "Ollama",
};

/** An AI tool's name from its ID ("codex" → "Codex"); nothing for nothing. */
export function toolName(id: string | null | undefined): string | null {
  return id ? (TOOL_NAMES[id] ?? id) : null;
}

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
    case "agent.memory_shortened":
      return brief(p.detail) || "The AI tool shortened its memory of this conversation";
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
    case "session.effort_changed":
      return str(p.effort)
        ? `The conversation's next tasks run at ${str(p.effort) === "xhigh" ? "extra high" : str(p.effort)} effort`
        : "The conversation's next tasks run at the AI tool's default effort";
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
    case "liaison.waiting_for_member": {
      // "waiting for Senior Developer to finish its current task"
      const reason = brief(p.reason, 200);
      return reason ? reason.charAt(0).toUpperCase() + reason.slice(1) : "Waiting its turn";
    }
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

/** The screenshot an Activity event carries, if any (capability use and approvals). */
export function eventScreenshot(payload: unknown): string | null {
  const p = (typeof payload === "object" && payload !== null ? payload : {}) as Record<
    string,
    unknown
  >;
  if (typeof p.screenshot === "string") return p.screenshot;
  const r = p.request;
  if (typeof r === "object" && r !== null) {
    const s = (r as Record<string, unknown>).screenshot;
    if (typeof s === "string") return s;
  }
  return null;
}

/**
 * Events the trail leaves out: a kept screenshot's own record, since the picture shows with the
 * action or approval it belongs to (the Ledger keeps both).
 */
export function shownInTrail(e: LedgerEvent): boolean {
  const p = (e.payload ?? {}) as Record<string, unknown>;
  return !(e.eventType === "artifact.recorded" && p.type === "screenshot");
}
