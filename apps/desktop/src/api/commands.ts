// Typed client for the Tauri command boundary.
// This is the ONLY module that calls `invoke`. Components import these functions,
// never `@tauri-apps/api/core` directly (enforced by ESLint).

import { invoke } from "@tauri-apps/api/core";
import type {
  ActivityScope,
  ActivitySeries,
  ApprovalQueue,
  AgentOverview,
  AgentRuntimeInfo,
  AgentSession,
  AgentSessionDetail,
  AppInfo,
  BackupInfo,
  BrowserChoice,
  BrowserStatus,
  ControlStatus,
  CommandError,
  CommandRules,
  GuardOptions,
  CommandErrorKind,
  DepartmentInput,
  DevelopmentInput,
  ExecutionOutput,
  ExecutionRecord,
  ExportInfo,
  HireInput,
  IntegrityReport,
  LedgerEvent,
  LedgerStatus,
  LiaisonOverview,
  ModelInput,
  ObjectiveReport,
  OrgSnapshot,
  OversightRole,
  PermissionSetInput,
  PermissionsSnapshot,
  PositionPatchInput,
  ProjectInput,
  ProjectWork,
  RoleInput,
  RolePolicy,
  RoleUpdate,
  RoutingOptions,
  RoutingSnapshot,
  RuntimeOverview,
  Screenshot,
  SecretInput,
  SensitiveKind,
  SensitiveRule,
  ServerIdentity,
  ServerInput,
  ServersSnapshot,
  ServerTest,
  SyntheticTaskAction,
  Task,
  TaskHandoffs,
  TaskTimeline,
  TaskTree,
  TitleTheme,
  WebsiteRules,
  Switches,
  LearningSnapshot,
  WorkView,
} from "@plenipo/types";

/** Error thrown by every command wrapper. Mirrors the Rust `CommandError` DTO. */
export class PlenipoCommandError extends Error {
  readonly kind: CommandErrorKind;

  constructor(kind: CommandErrorKind, message: string) {
    super(message);
    this.name = "PlenipoCommandError";
    this.kind = kind;
  }
}

function isCommandError(value: unknown): value is CommandError {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return (v.kind === "invalidInput" || v.kind === "internal") && typeof v.message === "string";
}

/** Normalize anything a failed `invoke` rejects with into a PlenipoCommandError. */
export function toCommandError(reason: unknown): PlenipoCommandError {
  if (reason instanceof PlenipoCommandError) return reason;
  if (isCommandError(reason)) return new PlenipoCommandError(reason.kind, reason.message);
  const message =
    reason instanceof Error
      ? reason.message
      : typeof reason === "string"
        ? reason
        : "Unknown error";
  return new PlenipoCommandError("internal", message);
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (reason) {
    throw toCommandError(reason);
  }
}

export function getAppInfo(): Promise<AppInfo> {
  return call<AppInfo>("get_app_info");
}

/** Signals that the shell rendered. Used by the launch smoke test; a no-op otherwise. */
export function frontendReady(): Promise<void> {
  return call<void>("frontend_ready");
}

/** Launch profiles, execution history (newest first), active count, and notices. */
export function getRuntimeOverview(): Promise<RuntimeOverview> {
  return call<RuntimeOverview>("get_runtime_overview");
}

/** Start an approved launch profile. The UI can never supply a command or path. */
export function startExecution(profileId: string): Promise<ExecutionRecord> {
  return call<ExecutionRecord>("start_execution", { profileId });
}

/** Terminate an execution's process tree; resolves with its final record. */
export function cancelExecution(executionId: string): Promise<ExecutionRecord> {
  return call<ExecutionRecord>("cancel_execution", { executionId });
}

/** Buffered output, used to rebuild the view after navigation or a reload. */
export function getExecutionOutput(executionId: string): Promise<ExecutionOutput> {
  return call<ExecutionOutput>("get_execution_output", { executionId });
}

// ---- Ledger -------------------------------------------------------------------------------

export function getLedgerStatus(): Promise<LedgerStatus> {
  return call<LedgerStatus>("get_ledger_status");
}

/** Tasks, newest first. */
export function listTasks(): Promise<Task[]> {
  return call<Task[]>("list_tasks");
}

/** A task's complete ordered activity trail and its direct children. */
export function getTaskTimeline(taskId: string): Promise<TaskTimeline> {
  return call<TaskTimeline>("get_task_timeline", { taskId });
}

/** Most recent ledger events, newest first. */
export function listRecentEvents(): Promise<LedgerEvent[]> {
  return call<LedgerEvent[]>("list_recent_events");
}

/**
 * Activity for each scope, counted into `buckets` time buckets over [from, to) (Phase 12A:
 * the activity strips; 96 buckets over 24 hours is 15 minutes each).
 */
export function getActivity(
  scopes: ActivityScope[],
  from: number,
  to: number,
  buckets = 96,
): Promise<ActivitySeries[]> {
  return call<ActivitySeries[]>("get_activity", { scopes, from, to, buckets });
}

/** Diagnostics: create a synthetic task. */
export function createSyntheticTask(): Promise<Task> {
  return call<Task>("create_synthetic_task");
}

/** Diagnostics: act on a synthetic task. The ledger's state machine decides what is allowed. */
export function advanceSyntheticTask(taskId: string, action: SyntheticTaskAction): Promise<Task> {
  return call<Task>("advance_synthetic_task", { taskId, action });
}

export function runIntegrityCheck(): Promise<IntegrityReport> {
  return call<IntegrityReport>("run_integrity_check");
}

/** Verified backup; Core chooses the location. */
export function createLedgerBackup(): Promise<BackupInfo> {
  return call<BackupInfo>("create_ledger_backup");
}

/** JSON export; Core chooses the location. */
export function exportLedger(): Promise<ExportInfo> {
  return call<ExportInfo>("export_ledger");
}

// ---- Agent runtimes (Phase 3) -------------------------------------------------------------

/** Runtimes (installation, sign-in, capabilities), sessions (newest first), and notices. */
export function getAgentOverview(): Promise<AgentOverview> {
  return call<AgentOverview>("get_agent_overview");
}

/** Re-detect every runtime's installation and sign-in. */
export function refreshAgentRuntimes(): Promise<AgentRuntimeInfo[]> {
  return call<AgentRuntimeInfo[]>("refresh_agent_runtimes");
}

/** A session with its turns and recent live activity. */
export function getAgentSession(sessionId: string): Promise<AgentSessionDetail> {
  return call<AgentSessionDetail>("get_agent_session", { sessionId });
}

/**
 * Start a session on a runtime with a first objective. The objective is sent to the runtime on
 * stdin by Core; the UI never supplies a command, path, or flag. With `handoffs`, the worker may
 * ask other workers for help through Plenipo Liaison.
 */
export function startAgentSession(
  runtimeId: string,
  objective: string,
  model?: string,
  handoffs = false,
): Promise<AgentSessionDetail> {
  return call<AgentSessionDetail>("start_agent_session", {
    runtimeId,
    objective,
    model: model?.trim() ? model.trim() : null,
    handoffs,
  });
}

/**
 * Give an existing session its next objective (resumes the provider session). A handoff worker's
 * session takes work only through Liaison.
 */
export function resumeAgentSession(
  sessionId: string,
  objective: string,
): Promise<AgentSessionDetail> {
  return call<AgentSessionDetail>("resume_agent_session", { sessionId, objective });
}

/**
 * Cancel the session's running turn, or end a turn waiting for handoff replies; resolves once the
 * turn is recorded. Liaison then stops the handoffs it was waiting for.
 */
export function cancelAgentTurn(sessionId: string): Promise<AgentSessionDetail> {
  return call<AgentSessionDetail>("cancel_agent_turn", { sessionId });
}

export function closeAgentSession(sessionId: string): Promise<AgentSession> {
  return call<AgentSession>("close_agent_session", { sessionId });
}

// ---- Liaison (Phase 4) --------------------------------------------------------------------

/** The handoff that created a task (if any) and the handoffs it made, with their replies. */
export function getTaskHandoffs(taskId: string): Promise<TaskHandoffs> {
  return call<TaskHandoffs>("get_task_handoffs", { taskId });
}

/** A task's whole delegation tree, from its root task, depth-first. */
export function getTaskTree(taskId: string): Promise<TaskTree> {
  return call<TaskTree>("get_task_tree", { taskId });
}

/** Liaison's protocol, limits, destinations, open handoffs, and notices. */
export function getLiaisonOverview(): Promise<LiaisonOverview> {
  return call<LiaisonOverview>("get_liaison_overview");
}

// ---- Workforce (Phase 5) ------------------------------------------------------------------
// Every change returns the organization as it is afterwards. The Ledger enforces the
// structure; a refused change rejects with the reason.

/** Departments, projects, positions with live status, oversight, and stats. */
export function getOrganization(): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("get_organization");
}

/** The work a position owns and its team's unfinished work; omit the ID for the organization. */
export function getWork(positionId?: string): Promise<WorkView> {
  return call<WorkView>("get_work", { positionId: positionId ?? null });
}

export function renameOrganization(name: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("rename_organization", { name });
}

/** Choose what the app calls the ranks. Display only: agents keep the plain titles. */
export function setOrganizationTitles(titles: TitleTheme): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("set_organization_titles", { titles });
}

export function createRole(input: RoleInput): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("create_role", { input });
}

/** Change a role you created: its name, description, and what it does (ADR-019). */
export function updateRole(roleId: string, input: RoleUpdate): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("update_role", { roleId, input });
}

/** Create a department with its head position. */
export function createDepartment(input: DepartmentInput): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("create_department", { input });
}

export function updateDepartment(
  departmentId: string,
  input: DepartmentInput,
): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("update_department", { departmentId, input });
}

/** Delete a department that has no projects (its head position is archived). */
export function removeDepartment(departmentId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("remove_department", { departmentId });
}

/** Create a project with its coordinator position. */
export function createProject(input: ProjectInput): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("create_project", { input });
}

export function updateProject(projectId: string, input: ProjectInput): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("update_project", { projectId, input });
}

/** Archive a project and its whole team. */
export function archiveProject(projectId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("archive_project", { projectId });
}

/** Hire into a team: a new position (and, for a persistent one, its agent). */
export function hirePosition(input: HireInput): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("hire_position", { input });
}

/** Hire an agent into a vacant persistent position. */
export function fillPosition(positionId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("fill_position", { positionId });
}

/** Let a persistent position's agent go; the position stays, vacant. */
export function vacatePosition(positionId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("vacate_position", { positionId });
}

export function updatePosition(
  positionId: string,
  input: PositionPatchInput,
): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("update_position", { positionId, input });
}

/** Make a position report to another (`null`: the owner). */
export function movePosition(positionId: string, reportsTo: string | null): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("move_position", { positionId, reportsTo });
}

export function archivePosition(positionId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("archive_position", { positionId });
}

/** Assign an on-demand position to review, QA, or security-audit a team. */
export function assignOversight(
  overseerId: string,
  targetId: string,
  role: OversightRole,
): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("assign_oversight", { overseerId, targetId, role });
}

export function endOversight(oversightId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("end_oversight", { oversightId });
}

/**
 * Give a staffed persistent position's agent an objective. Core builds its instructions and
 * chooses its session; the UI names only the position. With `projectId`, the objective belongs
 * to that project (which the position's team must run).
 */
export function giveObjective(
  positionId: string,
  objective: string,
  projectId?: string,
): Promise<AgentSessionDetail> {
  return call<AgentSessionDetail>("give_objective", {
    positionId,
    objective,
    ...(projectId ? { projectId } : {}),
  });
}

// ---- Development and projects (Phase 8) ---------------------------------------------------

/** Set up a software project in the Development department: the department and its VP when
 * missing, then the project with its Supervisor and the standard team. */
export function setUpDevelopment(input: DevelopmentInput): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("set_up_development", { input });
}

/** Plenipo's result for an objective (any task of it), built from the Ledger. */
export function getObjectiveReport(taskId: string): Promise<ObjectiveReport> {
  return call<ObjectiveReport>("get_objective_report", { taskId });
}

/** A project's objectives and working copies. */
export function getProjectWork(projectId: string): Promise<ProjectWork> {
  return call<ProjectWork>("get_project_work", { projectId });
}

/** Remove a working copy's folder (its branch stays in the repository). */
export function removeWorkspace(workspaceId: string): Promise<ProjectWork> {
  return call<ProjectWork>("remove_workspace", { workspaceId });
}

// ---- Model policy and routing (Phase 6) ---------------------------------------------------

/** Models, AI tools, every role's model choices with where its next worker would go. */
export function getRouting(): Promise<RoutingSnapshot> {
  return call<RoutingSnapshot>("get_routing");
}

/** Add a model (no `id`) or change one. */
export function saveModel(input: ModelInput): Promise<RoutingSnapshot> {
  return call<RoutingSnapshot>("save_model", { input });
}

/** Remove a model you added; it leaves every role's list. */
export function removeModel(modelId: string): Promise<RoutingSnapshot> {
  return call<RoutingSnapshot>("remove_model", { modelId });
}

export function setRolePolicy(roleId: string, policy: RolePolicy): Promise<RoutingSnapshot> {
  return call<RoutingSnapshot>("set_role_policy", { roleId, policy });
}

export function setRoutingOptions(options: RoutingOptions): Promise<RoutingSnapshot> {
  return call<RoutingSnapshot>("set_routing_options", { options });
}

/** Try an AI tool again now, although it reported a usage limit. */
export function clearUsageLimit(runtimeId: string): Promise<RoutingSnapshot> {
  return call<RoutingSnapshot>("clear_usage_limit", { runtimeId });
}

// ---- Permissions, approvals, and the Vault (Phase 7) --------------------------------------

/** Permission sets, who has which, the rules, secret references, and workers using
 * permissions now. */
export function getPermissions(): Promise<PermissionsSnapshot> {
  return call("get_permissions");
}

/** Add a permission set (no `id`) or change one. */
export function savePermissionSet(input: PermissionSetInput): Promise<PermissionsSnapshot> {
  return call("save_permission_set", { input });
}

export function removePermissionSet(setId: string): Promise<PermissionsSnapshot> {
  return call("remove_permission_set", { setId });
}

/** Give a role a permission set, or limit a department to one (`null`: none / no limit). */
export function assignPermissions(
  target: "role" | "department",
  id: string,
  setId: string | null,
): Promise<PermissionsSnapshot> {
  return call("assign_permissions", setId === null ? { target, id } : { target, id, setId });
}

export function setCommandRules(rules: CommandRules): Promise<PermissionsSnapshot> {
  return call("set_command_rules", { rules });
}

export function setBlockedFiles(patterns: string[]): Promise<PermissionsSnapshot> {
  return call("set_blocked_files", { patterns });
}

export function setSensitiveRule(
  kind: SensitiveKind,
  rule: SensitiveRule,
): Promise<PermissionsSnapshot> {
  return call("set_sensitive_rule", { kind, rule });
}

export function setGuardOptions(options: GuardOptions): Promise<PermissionsSnapshot> {
  return call("set_guard_options", { options });
}

/** Store a secret. Its value goes to the operating system's protected storage and is never
 * returned. */
export function saveSecret(input: SecretInput): Promise<PermissionsSnapshot> {
  return call("save_secret", { input });
}

export function removeSecret(secretId: string): Promise<PermissionsSnapshot> {
  return call("remove_secret", { secretId });
}

/** Approvals waiting for an answer and recent outcomes. */
export function getApprovals(): Promise<ApprovalQueue> {
  return call("get_approvals");
}

export function resolveApproval(approvalId: string, approve: boolean): Promise<ApprovalQueue> {
  return call("resolve_approval", { approvalId, approve });
}

/** End a worker's permissions now. */
export function revokeGrant(grantId: string): Promise<PermissionsSnapshot> {
  return call("revoke_grant", { grantId });
}

// ---- Browser and computer control (Phase 10) --------------------------------------------------

/** Who uses Plenipo's browser or the mouse and keyboard now, and whether control is stopped. */
export function getControlStatus(): Promise<ControlStatus> {
  return call("get_control_status");
}

/** The emergency stop: all browser, desktop, and server work halts, until you allow it again. */
export function stopAllControl(): Promise<ControlStatus> {
  return call("stop_all_control");
}

/** Take over a worker's use of the browser or the mouse and keyboard, or disconnect it from its
 * servers; that worker stops. */
export function takeOverControl(sessionId: string): Promise<ControlStatus> {
  return call("take_over_control", { sessionId });
}

/** Let workers use the browser, the desktop, and servers again after a stop. */
export function allowControl(): Promise<ControlStatus> {
  return call("allow_control");
}

export function setWebsiteRules(rules: WebsiteRules): Promise<PermissionsSnapshot> {
  return call("set_website_rules", { rules });
}

/** Learning's settings and the lessons waiting and kept (ADR-024). */
export function getLearning(): Promise<LearningSnapshot> {
  return call("get_learning");
}

/** Worker learning on or off. */
export function setLearning(enabled: boolean): Promise<LearningSnapshot> {
  return call("set_learning", { enabled });
}

/** Whether a role learns on its own (its lessons kept without asking). */
export function setRoleLearning(roleId: string, auto: boolean): Promise<LearningSnapshot> {
  return call("set_role_learning", { roleId, auto });
}

/** Keep a waiting lesson (in your own words, when `text` is given) or discard it. */
export function decideLesson(
  lessonId: string,
  keep: boolean,
  text?: string,
): Promise<LearningSnapshot> {
  return call("decide_lesson", { lessonId, keep, text: text ?? null });
}

/** Remove a kept lesson. */
export function removeLesson(lessonId: string): Promise<LearningSnapshot> {
  return call("remove_lesson", { lessonId });
}

/** The owner's on/off switches (ADR-023). Switching the browser or the screen off also stops
 * any worker using it now. */
export function setSwitches(switches: Switches): Promise<PermissionsSnapshot> {
  return call("set_switches", { switches });
}

export function getBrowserStatus(): Promise<BrowserStatus> {
  return call("get_browser_status");
}

/** Which browser is Plenipo's browser (ADR-028). An open one stays open; the choice is used from
 * its next start. */
export function setBrowserChoice(choice: BrowserChoice): Promise<BrowserStatus> {
  return call("set_browser_choice", { choice });
}

/** Open Plenipo's browser for you (to sign in to a website yourself). */
export function openBrowser(url?: string): Promise<BrowserStatus> {
  return call("open_browser", { url: url?.trim() ? url.trim() : null });
}

/** A kept screenshot, by its ID. */
export function getScreenshot(artifactId: string): Promise<Screenshot> {
  return call("get_screenshot", { artifactId });
}

// ---- Servers (Phase 11) ------------------------------------------------------------------------

/** Settings → Servers: the servers (never their keys or passwords), the roles, the kinds of
 * commands, and where sign-ins are kept. */
export function getServers(): Promise<ServersSnapshot> {
  return call("get_servers");
}

/** Add a server (no `id`) or change one. A key, passphrase, or password is sent once, kept in the
 * operating system's protected storage, and never returned. */
export function saveServer(input: ServerInput): Promise<ServersSnapshot> {
  return call("save_server", { input });
}

/** Remove a server and its sign-in; workers connected to it are disconnected. */
export function removeServer(serverId: string): Promise<ServersSnapshot> {
  return call("remove_server", { serverId });
}

/** Read a server's ID (its host key fingerprint) for you to check and pin; nothing signs in. */
export function checkServerIdentity(host: string, port: number): Promise<ServerIdentity> {
  return call("check_server_identity", { host, port });
}

/** Connect with the pinned server ID, sign in, and leave. */
export function testServer(serverId: string): Promise<ServerTest> {
  return call("test_server", { serverId });
}
