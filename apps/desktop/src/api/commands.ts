// Typed client for the Tauri command boundary.
// This is the ONLY module that calls `invoke`. Components import these functions,
// never `@tauri-apps/api/core` directly (enforced by ESLint).

import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  Safety,
  BlockedPerson,
  CardView,
  CommunityView,
  ConversationSummary,
  ConversationView,
  Found,
  GettingStarted,
  KeptOnPc,
  LeaderboardView,
  MessageView,
  PeoplePage,
  PointsView,
  RemoteSettings,
  ReportOf,
  LiveView,
  LoanUntil,
  OwnerProfile,
  OwnerProfileInput,
  TilePlace,
  WatchFileView,
  WatchUpdate,
  WatchView,
  DiagnosticsFile,
  LedgerBackups,
  RecoveryStatus,
  StartAndClose,
  StartAndCloseInput,
  UpdateStatus,
  ActivityScope,
  ActivitySeries,
  ApprovalQueue,
  AgentOverview,
  AgentRuntimeInfo,
  AgentSession,
  AiToolsPage,
  AiToolUsage,
  PaymentMethod,
  CapCovers,
  SpendingPage,
  Access,
  AccountKind,
  AppInput,
  AddOnChange,
  AddOnInput,
  ConnectionsPage,
  KeyInput,
  ToolMark,
  OwnApp,
  Part,
  Service,
  PartLevel,
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
  ProfileDraft,
  ShareProfile,
  ProjectInput,
  ProjectWork,
  RoleInput,
  RolePolicy,
  RoleUpdate,
  RuleTarget,
  ModelRule,
  DeletionPreview,
  SpecialtyInput,
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
  HomeView,
  LocalPath,
  NoticeSettings,
  TaskRecord,
  WorkRecord,
  TerminalEvent,
  TerminalInfo,
  TerminalPlace,
  TerminalSettings,
  TerminalShell,
  PanelId,
  WindowPlace,
  ChangingFile,
  FileRoots,
  WorkFolder,
  ChainOrder,
  FileView,
  FolderListing,
  LineEnding,
  ObjectiveFile,
  SaveOutcome,
  OrgDeletePreview,
  OrgListing,
  OrgOpened,
  OrgStart,
  OrgSummary,
  LicenseView,
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
  return (
    (v.kind === "invalidInput" || v.kind === "internal" || v.kind === "partOfPro") &&
    typeof v.message === "string"
  );
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

/** Archive a department with everything in it (ADR-043), once nothing in it has unfinished work. */
export function archiveDepartment(departmentId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("archive_department", { departmentId });
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

// ---- Archive, bring back, delete for good, and the Workforce (Phase 17) -------------------

/** What can be brought back or deleted for good. */
export type ArchivedKind = "position" | "project" | "department";

/** Bring an archived agent, project, or department back as it was (ADR-043). */
export function bringBack(kind: ArchivedKind, id: string): Promise<OrgSnapshot> {
  switch (kind) {
    case "position":
      return call<OrgSnapshot>("bring_back_position", { positionId: id });
    case "project":
      return call<OrgSnapshot>("bring_back_project", { projectId: id });
    case "department":
      return call<OrgSnapshot>("bring_back_department", { departmentId: id });
  }
}

/** What deleting an archived item for good would take along, for the confirmation. */
export function previewDeleteForGood(kind: ArchivedKind, id: string): Promise<DeletionPreview> {
  return call<DeletionPreview>("preview_delete_for_good", { kind, id });
}

/**
 * Delete an archived item for good, after you confirmed. The agents in `save` move to your
 * Workforce instead (ADR-045). Refused while anything has unfinished work.
 */
export function deleteForGood(
  kind: ArchivedKind,
  id: string,
  save: string[],
): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("delete_for_good", { kind, id, save });
}

/** Save an archived agent to your Workforce. */
export function saveToWorkforce(positionId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("save_to_workforce", { positionId });
}

/** Hire an agent from your Workforce into a team (`reportsTo` null: you). */
export function hireFromWorkforce(
  savedId: string,
  reportsTo: string | null,
  title?: string,
): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("hire_from_workforce", {
    savedId,
    reportsTo,
    title: title?.trim() ? title.trim() : null,
  });
}

/** Delete an agent in your Workforce for good. */
export function deleteSavedAgent(savedId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("delete_saved_agent", { savedId });
}

// ---- The organization canvas (Phase 18, ADR-053) --------------------------------------------

/** Save where you put these tiles (a whole team moved at once). */
export function placeTiles(places: TilePlace[]): Promise<void> {
  return call<void>("place_tiles", { places });
}

/** Tidy up: forget every place; resolves with them, for Undo. */
export function tidyUp(): Promise<TilePlace[]> {
  return call<TilePlace[]>("tidy_up");
}

/** Move one end of an oversight line: another overseer, or another team. */
export function retargetOversight(
  oversightId: string,
  to: { overseerId: string } | { targetId: string },
): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("retarget_oversight", {
    oversightId,
    overseerId: "overseerId" in to ? to.overseerId : null,
    targetId: "targetId" in to ? to.targetId : null,
  });
}

/** Who is working where now, what each touched last, and recent hand-offs. */
export function getLiveView(): Promise<LiveView> {
  return call<LiveView>("get_live_view");
}

// ---- Move or lend (Phase 18, ADR-054) -------------------------------------------------------

/** Lend an on-call agent to another team's lead. */
export function lendAgent(
  positionId: string,
  toLeadId: string,
  until: LoanUntil,
): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("lend_agent", { positionId, toLeadId, until });
}

/** Send a lent agent home: now, or when its task ends. */
export function sendHome(positionId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("send_home", { positionId });
}

// ---- Watch (Phase 18, ADR-055) — read-only --------------------------------------------------

/** The files an agent's workers changed in its latest objective. */
export function getWatch(positionId: string): Promise<WatchView> {
  return call<WatchView>("get_watch", { positionId });
}

/** One change's file, its lines marked; `null` when Plenipo no longer has it. */
export function getWatchChange(changeId: string): Promise<WatchFileView | null> {
  return call<WatchFileView | null>("get_watch_change", { changeId });
}

/**
 * Hear each file change a worker is writing, saved, or refused, through a channel only this
 * window hears (never an event, which another window could listen to). Resolves with the
 * function that stops it.
 */
export async function watchChanges(onUpdate: (update: WatchUpdate) => void): Promise<() => void> {
  const updates = new Channel<WatchUpdate>();
  let hearing = true;
  updates.onmessage = (update) => {
    if (hearing) onUpdate(update);
  };
  const subscription = await call<number>("subscribe_watch", { channel: updates });
  return () => {
    hearing = false;
    void call<void>("unsubscribe_watch", { subscription }).catch(() => undefined);
  };
}

// ---- The owner's tile (Phase 18, ADR-056) ---------------------------------------------------

export function getOwnerProfile(): Promise<OwnerProfile> {
  return call<OwnerProfile>("get_owner_profile");
}

export function setOwnerProfile(input: OwnerProfileInput): Promise<OwnerProfile> {
  return call<OwnerProfile>("set_owner_profile", { input });
}

// ---- Specialties (Phase 17, ADR-042) --------------------------------------------------------

/** Add one of your own specialties to a role. */
export function createSpecialty(input: SpecialtyInput): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("create_specialty", { input });
}

/** Change one of your own specialties. */
export function updateSpecialty(specialtyId: string, input: SpecialtyInput): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("update_specialty", { specialtyId, input });
}

/** Remove one of your own specialties (refused while an agent on the chart has it). */
export function removeSpecialty(specialtyId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("remove_specialty", { specialtyId });
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
  files?: readonly ObjectiveFile[],
): Promise<AgentSessionDetail> {
  return call<AgentSessionDetail>("give_objective", {
    positionId,
    objective,
    ...(projectId ? { projectId } : {}),
    ...(files && files.length > 0 ? { files } : {}),
  });
}

// ---- Development and projects (Phase 8) ---------------------------------------------------

/** Set up a software project in the Development department: the department and its VP when
 * missing, then the project with its Supervisor and the standard team. */
export function setUpDevelopment(input: DevelopmentInput): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("set_up_development", { input });
}

/**
 * A position's chain of command (ADR-202): the owner's orders it was given, that went through
 * it, or that went past it, newest first, with where each stands and what came back up to it.
 */
export function getChainOrders(positionId: string): Promise<ChainOrder[]> {
  return call<ChainOrder[]>("get_chain_orders", { positionId });
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

/**
 * Set the organization's, a department's, or one agent's model and effort rule (ADR-041). An
 * empty rule for a department or an agent removes it.
 */
export function setModelRule(target: RuleTarget, rule: ModelRule): Promise<RoutingSnapshot> {
  return call<RoutingSnapshot>("set_model_rule", { target, rule });
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

/** Learning on or off for a role's agents (ADR-041). */
export function setRoleLearns(roleId: string, learns: boolean): Promise<LearningSnapshot> {
  return call("set_role_learns", { roleId, learns });
}

/** Learning on or off for one agent; `null`: it follows its role. */
export function setAgentLearning(
  positionId: string,
  learns: boolean | null,
): Promise<LearningSnapshot> {
  return call("set_agent_learning", { positionId, learns });
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

/** How much Plenipo asks before an agent saves files or runs programs (ADR-201). */
export function setSafety(safety: Safety): Promise<PermissionsSnapshot> {
  return call("set_safety", { safety });
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

/**
 * The owner's Stop in a worker's watch tab (Phase 12, ADR-031): the command running now is sent
 * TERM, then KILL. The worker's step goes on (Disconnect ends it: `takeOverControl`).
 */
export function stopServerCommand(commandId: string): Promise<void> {
  return call<void>("stop_server_command", { commandId });
}

// ---- The owner's terminal (Phase 12, ADR-031) ---------------------------------------------------

/** Settings → Terminal: the shell for this PC, the choices, and the terminals open now. */
export function getTerminalSettings(): Promise<TerminalSettings> {
  return call<TerminalSettings>("get_terminal_settings");
}

/** Choose the shell a new terminal on this PC starts. */
export function setTerminalShell(shell: TerminalShell): Promise<TerminalSettings> {
  return call<TerminalSettings>("set_terminal_shell", { shell });
}

/**
 * Open a terminal for the owner, on this PC or on a server. What it shows arrives through
 * `onEvent`, as it happens (it can start before the promise resolves).
 */
export function openTerminal(
  place: TerminalPlace,
  cols: number,
  rows: number,
  onEvent: (event: TerminalEvent) => void,
): Promise<TerminalInfo> {
  const events = new Channel<TerminalEvent>();
  events.onmessage = onEvent;
  return call<TerminalInfo>("open_terminal", { place, cols, rows, events });
}

/** What the owner types (or pastes). */
export function writeTerminal(terminalId: string, data: string): Promise<void> {
  return call<void>("write_terminal", { terminalId, data });
}

/** The terminal's size changed (in characters). */
export function resizeTerminal(terminalId: string, cols: number, rows: number): Promise<void> {
  return call<void>("resize_terminal", { terminalId, cols, rows });
}

/** Close a terminal: its shell, and the programs it started, end. */
export function closeTerminal(terminalId: string): Promise<void> {
  return call<void>("close_terminal", { terminalId });
}

// ---- The pages (Phase 12) --------------------------------------------------------------------------

/** Home: objectives still going, those finished in the last week with their answers, and what is stuck. */
export function getHome(): Promise<HomeView> {
  return call<HomeView>("get_home");
}

/**
 * A department's, project's, or position's history (or everything): newest first, before event
 * `before` for the next page.
 */
export function getScopeEvents(
  scope: ActivityScope,
  limit = 50,
  before?: number,
): Promise<LedgerEvent[]> {
  return call<LedgerEvent[]>("get_scope_events", { scope, limit, before: before ?? null });
}

/** A task's events with those of every task under it, newest first. */
export function getTaskEvents(taskId: string, limit = 50, before?: number): Promise<LedgerEvent[]> {
  return call<LedgerEvent[]>("get_task_events", { taskId, limit, before: before ?? null });
}

/** A project's pull requests, artifacts, and recent decisions. */
export function getProjectRecord(projectId: string): Promise<WorkRecord> {
  return call<WorkRecord>("get_project_record", { projectId });
}

/** A task's pull requests, artifacts, decisions, and approvals (with the tasks under it). */
export function getTaskRecord(taskId: string): Promise<TaskRecord> {
  return call<TaskRecord>("get_task_record", { taskId });
}

// ---- Notices (Phase 12) ----------------------------------------------------------------------

/** Settings → Notifications: which pop-up notices you get, and when. */
export function getNoticeSettings(): Promise<NoticeSettings> {
  return call<NoticeSettings>("get_notice_settings");
}

/** Keep your choices for pop-up notices. */
export function setNoticeSettings(settings: NoticeSettings): Promise<NoticeSettings> {
  return call<NoticeSettings>("set_notice_settings", { settings });
}

/** Show a notice now, to check that the computer shows Plenipo's notices. */
export function sendTestNotice(): Promise<void> {
  return call<void>("send_test_notice");
}

/** Settings → Local paths: where Plenipo keeps its files on this computer (read only). */
export function getLocalPaths(): Promise<LocalPath[]> {
  return call<LocalPath[]>("get_local_paths");
}

// ---- Keeping Plenipo dependable (Phase 13) ------------------------------------------------

/** How the last run ended and what stopped, a window that was brought back, and settings
 * Plenipo could not read. */
export function getRecoveryStatus(): Promise<RecoveryStatus> {
  return call<RecoveryStatus>("get_recovery_status");
}

/** Give a stopped objective to the same worker again (through the usual checks). */
export function runAgain(taskId: string): Promise<RecoveryStatus> {
  return call<RecoveryStatus>("run_again", { taskId });
}

/** Leave the stopped tasks stopped: the notice about the last run goes away. */
export function dismissRecovery(id: string): Promise<RecoveryStatus> {
  return call<RecoveryStatus>("dismiss_recovery", { id });
}

/** You read that the window was brought back. */
export function dismissWindowRecovery(): Promise<RecoveryStatus> {
  return call<RecoveryStatus>("dismiss_window_recovery");
}

/** The window's page is alive (every few seconds). */
export function windowAlive(visible: boolean): Promise<void> {
  return call<void>("window_alive", { visible });
}

/** Reset settings Plenipo could not read to their starting values (a backup is made first). */
export function resetSettings(key: string): Promise<RecoveryStatus> {
  return call<RecoveryStatus>("reset_settings", { key });
}

/** Settings → Start and close. */
export function getStartAndClose(): Promise<StartAndClose> {
  return call<StartAndClose>("get_start_and_close");
}

/** Change Settings → Start and close. */
export function setStartAndClose(input: StartAndCloseInput): Promise<StartAndClose> {
  return call<StartAndClose>("set_start_and_close", { input });
}

/** Every backup of the Ledger, newest first, and a restore waiting for the next start. */
export function listLedgerBackups(): Promise<LedgerBackups> {
  return call<LedgerBackups>("list_ledger_backups");
}

/** Restore the Ledger from a backup named in the list. Plenipo restarts to do it. */
export function restoreLedgerBackup(name: string): Promise<LedgerBackups> {
  return call<LedgerBackups>("restore_ledger_backup", { name });
}

/** Forget a restore that has not happened yet. */
export function cancelLedgerRestore(): Promise<LedgerBackups> {
  return call<LedgerBackups>("cancel_ledger_restore");
}

/** Save a diagnostics file (Core chooses where) to send when something went wrong. */
export function saveDiagnosticsFile(): Promise<DiagnosticsFile> {
  return call<DiagnosticsFile>("save_diagnostics_file");
}

/** Settings → Updates. */
export function getUpdateStatus(): Promise<UpdateStatus> {
  return call<UpdateStatus>("get_update_status");
}

/** Check GitHub now for a newer version. */
export function checkForUpdates(): Promise<UpdateStatus> {
  return call<UpdateStatus>("check_for_updates");
}

/** Install the newer version. `stopWork`: you agreed that running work stops. Plenipo quits
 * and the installer opens the new version. */
export function installUpdate(stopWork: boolean): Promise<UpdateStatus> {
  return call<UpdateStatus>("install_update", { stopWork });
}

// ---- The AI tools page (Phase 19, ADR-058 to ADR-060) ---------------------------------------

/** Each AI tool's newest version, update, plan left, and how it is paid for, and the switch. */
export function getAiTools(): Promise<AiToolsPage> {
  return call<AiToolsPage>("get_ai_tools");
}

/** Check one AI tool again: its version, sign-in, models, and (where it reports it) plan. */
export function checkAiTool(runtimeId: string): Promise<AiToolsPage> {
  return call<AiToolsPage>("check_ai_tool", { runtimeId });
}

/** Look for each AI tool's newest version now. */
export function checkAiToolVersions(): Promise<AiToolsPage> {
  return call<AiToolsPage>("check_ai_tool_versions");
}

/**
 * An AI tool's usage, day by day: `dayStarts` are the days' starts, in order (midnight on this
 * computer), and the last one ends the last day. At most 60 days.
 */
export function getAiToolUsage(runtimeId: string, dayStarts: number[]): Promise<AiToolUsage> {
  return call<AiToolUsage>("get_ai_tool_usage", { runtimeId, dayStarts });
}

/** Update an AI tool with its own update command. It waits while a task is using the tool. */
export function updateAiTool(runtimeId: string): Promise<AiToolsPage> {
  return call<AiToolsPage>("update_ai_tool", { runtimeId });
}

/** Stop an update that is still waiting for its AI tool to be free. */
export function cancelAiToolUpdate(runtimeId: string): Promise<AiToolsPage> {
  return call<AiToolsPage>("cancel_ai_tool_update", { runtimeId });
}

/** The switch: update AI tools by themselves, or ask first (the default). */
export function setAiToolsAutoUpdate(on: boolean): Promise<AiToolsPage> {
  return call<AiToolsPage>("set_ai_tools_auto_update", { on });
}

/**
 * How an AI tool is paid for. A subscription AI tool always uses its subscription; a paid AI tool
 * (OpenRouter) is paid per use with the owner's key (Phase 16 Wave 3, ADR-085).
 */
export function setAiToolPayment(runtimeId: string, method: PaymentMethod): Promise<AiToolsPage> {
  return call<AiToolsPage>("set_ai_tool_payment", { runtimeId, method });
}

/**
 * Save a paid AI tool's key (ADR-085): Plenipo checks it with the AI company, then keeps it in
 * the Vault. The key is typed only on this screen; no answer repeats it.
 */
export function savePaidKey(runtimeId: string, name: string, key: string): Promise<AiToolsPage> {
  return call<AiToolsPage>("save_paid_key", { runtimeId, name, key });
}

/** Remove a paid AI tool's key, from the Vault too. */
export function removePaidKey(runtimeId: string): Promise<AiToolsPage> {
  return call<AiToolsPage>("remove_paid_key", { runtimeId });
}

// ---- Settings → Spending caps (Phase 16 Wave 3, ADR-085) --------------------------------------
// Money is whole millionths of a dollar ("micros"). None of these spends money or takes a key.

/** This month's spending, each cap, and the month's latest paid tasks. */
export function getSpending(): Promise<SpendingPage> {
  return call<SpendingPage>("get_spending");
}

/** Set (or change) the monthly cap for the business, a department, or one position. */
export function setSpendingCap(covers: CapCovers, monthlyMicros: number): Promise<SpendingPage> {
  return call<SpendingPage>("set_spending_cap", { covers, monthlyMicros });
}

/** Remove a cap. */
export function removeSpendingCap(capId: string): Promise<SpendingPage> {
  return call<SpendingPage>("remove_spending_cap", { capId });
}

// ---- Settings → Connections (Phase 20, ADR-062 to ADR-065) ------------------------------------
// None of these takes a password, a key, or a token: signing in happens on the service's own
// page in your browser, and its sign-in is kept only in the Vault.

/** Each service, its connection's state, parts, who may use it, and its list. */
export function getConnections(): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("get_connections");
}

/** Open the service's sign-in page in your browser; Plenipo waits for it in the background. */
export function connectConnection(
  connectionId: string,
  kind: AccountKind,
): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("connect_connection", { connectionId, kind });
}

/** Stop a sign-in still waiting in your browser. */
export function cancelConnectionSignIn(connectionId: string): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("cancel_connection_sign_in", { connectionId });
}

/** Disconnect: its tools stop at once, and its sign-in leaves the Vault. */
export function disconnectConnection(connectionId: string): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("disconnect_connection", { connectionId });
}

/** Parts' levels (those not given keep theirs). */
export function setConnectionParts(
  connectionId: string,
  parts: Partial<Record<Part, PartLevel>>,
): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("set_connection_parts", { connectionId, parts });
}

/** **Who may use it**: roles and agents, each Read only or Read and write. */
export function setConnectionAccess(
  connectionId: string,
  access: Access[],
): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("set_connection_access", { connectionId, access });
}

/** **Send without asking to**: addresses, `@domains`, and channels. */
export function setConnectionSendList(
  connectionId: string,
  list: string[],
): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("set_connection_send_list", { connectionId, list });
}

/** Advanced: the organization's own app ID (not a secret), or 8 West's (`null`). */
export function setConnectionOwnApp(
  connectionId: string,
  app: OwnApp | null,
): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("set_connection_own_app", { connectionId, app });
}

/**
 * The owner's own Slack app (its client ID) or Google app (its client ID and secret), or none
 * (`null`). A Google app's secret goes straight to the Vault; nothing ever sends it back.
 */
export function saveConnectionApp(
  connectionId: string,
  app: AppInput | null,
): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("save_connection_app", { connectionId, app });
}

/** Add another account of a service that may have more than one (a Slack workspace). */
export function addConnection(service: Service): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("add_connection", { service });
}

/** Remove a card that is not connected (a Slack workspace). */
export function removeConnection(connectionId: string): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("remove_connection", { connectionId });
}

/**
 * Save and check a key typed into a card (HubSpot, Stripe, the website; ADR-071): checked with one
 * reading call, kept only in the Vault if the service accepts it, and never returned.
 */
export function saveConnectionKey(connectionId: string, key: KeyInput): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("save_connection_key", { connectionId, key });
}

/** Add a program that offers tools (ADR-066): off, with no tool marked and nobody allowed. */
export function addAddOn(addOn: AddOnInput): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("add_add_on", { addOn });
}

/** Change an add-on (switching it on looks at its tools first). */
export function changeAddOn(addOnId: string, change: AddOnChange): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("change_add_on", { addOnId, change });
}

/** Remove an add-on. */
export function removeAddOn(addOnId: string): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("remove_add_on", { addOnId });
}

/** Start the program once and list its tools: new or changed ones are Off. */
export function checkAddOnTools(addOnId: string): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("check_add_on_tools", { addOnId });
}

/** Mark an add-on's tools Off, Reading, or Changing (by the program's names for them). */
export function setAddOnTools(
  addOnId: string,
  marks: Record<string, ToolMark>,
): Promise<ConnectionsPage> {
  return call<ConnectionsPage>("set_add_on_tools", { addOnId, marks });
}

// ---- The workspace (Phase 21, ADR-092) ------------------------------------------------------

/**
 * Tell Plenipo this window's page is about to open `panel` in its own window (`place`: where the
 * panel was dropped; none for where it was last). Plenipo allows the next new window of this
 * page for that panel only, once.
 */
export function preparePopOut(panel: PanelId, place: WindowPlace | null): Promise<void> {
  return call<void>("prepare_pop_out", { panel, place });
}

/** Bring a popped-out panel's window to the front. `false` when it is not open. */
export function focusPopOut(panel: PanelId): Promise<boolean> {
  return call<boolean>("focus_pop_out", { panel });
}

/** Put back: close this window's pop-out of `panel`. `false` when none was open. */
export function closePopOut(panel: PanelId): Promise<boolean> {
  return call<boolean>("close_pop_out", { panel });
}

/** Reset layout: close this window's pop-outs and forget where they were. */
export function resetPopOuts(): Promise<void> {
  return call<void>("reset_pop_outs");
}

// ---- The owner's files (Phase 21, ADR-093) --------------------------------------------------

/** The folders Plenipo knows: each project's folder and working copies, and who writes where. */
export function getFileRoots(): Promise<FileRoots> {
  return call<FileRoots>("get_file_roots");
}

/** One folder's files and folders (`path`: inside the top folder, "" for its top). */
export function listFolder(root: string, path: string): Promise<FolderListing> {
  return call<FolderListing>("list_folder", { root, path });
}

/** Open a file in Plenipo. */
export function readFile(root: string, path: string): Promise<FileView> {
  return call<FileView>("read_file", { root, path });
}

/**
 * Save a text file (`base`: its fingerprint when it was opened; `null`: Save anyway, over what is
 * there now). Its own line endings and byte-order mark are kept.
 */
export function saveFile(
  root: string,
  path: string,
  text: string,
  bom: boolean,
  lineEnding: LineEnding,
  base: string | null,
): Promise<SaveOutcome> {
  return call<SaveOutcome>("save_file", { root, path, text, bom, lineEnding, base });
}

/** Open a file with the program Windows uses for it (never a program or a script). */
export function openFileOutside(root: string, path: string): Promise<void> {
  return call<void>("open_file_outside", { root, path });
}

/** Show a file in File Explorer. */
export function showInFolder(root: string, path: string): Promise<void> {
  return call<void>("show_in_folder", { root, path });
}

/**
 * Where a task's worker kept its files (ADR-201): Plenipo's own folder in Documents, the
 * project's folder, or a working copy. `null` when it had no folder.
 */
export function getWorkFolder(taskId: string): Promise<WorkFolder | null> {
  return call<WorkFolder | null>("get_work_folder", { taskId });
}

/** Open the folder where a task's worker kept its files, in File Explorer. */
export function openWorkFolder(taskId: string): Promise<void> {
  return call<void>("open_work_folder", { taskId });
}

/** The files workers are changing now. */
export function getChangingFiles(): Promise<ChangingFile[]> {
  return call<ChangingFile[]>("get_changing_files");
}

// ---- More than one organization (Phase 21, ADR-094) ----------------------------------------

/** Your organizations, and which one this window shows. */
export function getOrganizations(): Promise<OrgListing> {
  return call<OrgListing>("get_organizations");
}

/** Make an organization: a template, a copy of another one's setup, or from scratch. */
export function createOrganization(name: string, start: OrgStart): Promise<OrgSummary> {
  return call<OrgSummary>("create_organization", { name, start });
}

/** Show another organization in this window (the page loads again). */
export function switchOrganization(id: string): Promise<OrgOpened> {
  return call<OrgOpened>("switch_organization", { id });
}

/** Open an organization in a window of its own (or bring its window to the front). */
export function openOrganizationWindow(id: string): Promise<OrgOpened> {
  return call<OrgOpened>("open_organization_window", { id });
}

/** Stop an organization's work and hide it from the list; Bring back opens it again. */
export function archiveOrganization(id: string): Promise<OrgListing> {
  return call<OrgListing>("archive_organization", { id });
}

export function bringBackOrganization(id: string): Promise<OrgListing> {
  return call<OrgListing>("bring_back_organization", { id });
}

/** What deleting an archived organization for good removes, and whom it can save. */
export function previewDeleteOrganization(id: string): Promise<OrgDeletePreview> {
  return call<OrgDeletePreview>("preview_delete_organization", { id });
}

/** Delete an archived organization for good, saving the workers in `save` to your Workforce. */
export function deleteOrganizationForGood(id: string, save: string[]): Promise<OrgListing> {
  return call<OrgListing>("delete_organization_for_good", { id, save });
}

// ---- Settings → License (Phase 11A, ADR-021, ADR-022) ----------------------------------------

/** The license on this PC: Free or Pro, why, and the last check with 8 West (never the key). */
export function getLicense(): Promise<LicenseView> {
  return call<LicenseView>("get_license");
}

/**
 * Enter a license key. It is checked on this PC, kept in the Vault, and never shown again; Pro
 * starts at once, and the first check with 8 West starts by itself.
 */
export function enterLicenseKey(key: string): Promise<LicenseView> {
  return call<LicenseView>("enter_license_key", { key });
}

/** Remove the license key: Free from now, with nothing else changed. */
export function removeLicenseKey(): Promise<LicenseView> {
  return call<LicenseView>("remove_license_key");
}

/** Check with 8 West now (sends the key's ID and this version; nothing without a key). */
export function checkLicenseNow(): Promise<LicenseView> {
  return call<LicenseView>("check_license_now");
}

// ---- Settings → Devices: Plenipo on your phone (Phase 14, ADR-141, ADR-145) -----------------

/** Phone access: the switch, the phones, adding one, and the approvals kept on this PC. */
export function getRemote(): Promise<RemoteSettings> {
  return call<RemoteSettings>("get_remote");
}

/** Settings → Switches → Use Plenipo from another device. Off cuts every phone off at once. */
export function setRemoteSwitch(on: boolean): Promise<RemoteSettings> {
  return call<RemoteSettings>("set_remote_switch", { on });
}

/** Add a phone: a new picture code and typed code, for 10 minutes. */
export function startPhonePairing(): Promise<RemoteSettings> {
  return call<RemoteSettings>("start_phone_pairing");
}

/** Stop adding a phone. */
export function cancelPhonePairing(): Promise<RemoteSettings> {
  return call<RemoteSettings>("cancel_phone_pairing");
}

/** The answer to "Is this your phone?": nothing is added until you say yes. */
export function answerPhonePairing(add: boolean): Promise<RemoteSettings> {
  return call<RemoteSettings>("answer_phone_pairing", { add });
}

export function renameDevice(id: string, name: string): Promise<RemoteSettings> {
  return call<RemoteSettings>("rename_device", { id, name });
}

/** Remove a phone: it is cut off at once. */
export function removeDevice(id: string): Promise<RemoteSettings> {
  return call<RemoteSettings>("remove_device", { id });
}

/** Un-pause a phone paused after failed checks. */
export function unpauseDevice(id: string): Promise<RemoteSettings> {
  return call<RemoteSettings>("unpause_device", { id });
}

/** Keep these approvals on my PC only. */
export function setKeptOnPc(kept: KeptOnPc): Promise<RemoteSettings> {
  return call<RemoteSettings>("set_kept_on_pc", { kept });
}

/** Settings → Notifications → **Notices on my phones** (Phase 14 part 14C). */
export function setPhoneNotices(on: boolean): Promise<RemoteSettings> {
  return call<RemoteSettings>("set_phone_notices", { on });
}

// ---- Settings → Community: your 8 West account in Plenipo (Phase 24, ADR-162, ADR-170) ------

/** Community as it is: the switch, where signing in stands, and who you are in it. */
export function getCommunity(): Promise<CommunityView> {
  return call<CommunityView>("get_community");
}

/**
 * Settings → Switches → Community. On asks 8 West whether Community is open and, if it is, shows
 * a code to sign in with. Off is Leave Community.
 */
export function setCommunitySwitch(on: boolean): Promise<CommunityView> {
  return call<CommunityView>("set_community_switch", { on });
}

/** Check again (after Coming soon) and Sign in (after signing out): the same as turning it on. */
export function checkCommunityAgain(): Promise<CommunityView> {
  return call<CommunityView>("check_community_again");
}

/** Stop signing in: nothing was kept. */
export function cancelCommunitySignIn(): Promise<CommunityView> {
  return call<CommunityView>("cancel_community_sign_in");
}

/**
 * Join Community: your Community name, your birth month and year (asked once, never the day), the
 * terms version you agreed to on screen, and your profile with its boxes (**What people see**,
 * ADR-163). Plenipo adds your tile's picture, status, mood, and message itself.
 */
export function joinCommunity(
  name: string,
  birthMonth: number,
  birthYear: number,
  terms: string,
  profile: ProfileDraft,
): Promise<CommunityView> {
  return call<CommunityView>("join_community", { name, birthMonth, birthYear, terms, profile });
}

/** Save your profile and its boxes (**Your profile**). An unticked part is hidden at once. */
export function saveCommunityProfile(profile: ProfileDraft): Promise<CommunityView> {
  return call<CommunityView>("save_community_profile", { profile });
}

/**
 * **Appear offline**, or not: you leave the directory, New this week, and the leaderboard, and
 * people you talk with see you as Offline.
 */
export function setCommunityAppearOffline(offline: boolean): Promise<CommunityView> {
  return call<CommunityView>("set_community_appear_offline", { offline });
}

/** Sign out of your 8 West account on this computer. You stay a member. */
export function signOutOfCommunity(): Promise<CommunityView> {
  return call<CommunityView>("sign_out_of_community");
}

/** Open the sign-in page, or the Community terms, in your own web browser. */
export function openCommunityPage(page: "signIn" | "terms"): Promise<void> {
  return call<void>("open_community_page", { page });
}

// ---- Community → People: finding people (Phase 24, ADR-163 §4, §6) ---------------------------

/**
 * **Directory**: 20 cards at a time. `q` is up to 60 characters (a name, a company, or what a
 * business does), `kind` one business kind (`""` for any), `region` a country (`US`) or a state
 * (`US-CA`), or `""` for anywhere. `cursor` is the `next` of the page before, or `""` for the first.
 */
export function communityDirectory(
  q: string,
  kind: string,
  region: string,
  cursor: string,
): Promise<PeoplePage> {
  return call<PeoplePage>("community_directory", { q, kind, region, cursor });
}

/** **New this week**: people who joined in the last 7 days, 20 at a time. */
export function communityNewThisWeek(cursor: string): Promise<PeoplePage> {
  return call<PeoplePage>("community_new_this_week", { cursor });
}

/**
 * **Find someone** by their exact Community name (an "@" before it is fine; Plenipo makes the
 * letters small). It answers a card, "request only", or no one, and nothing more.
 */
export function findInCommunity(name: string): Promise<Found> {
  return call<Found>("find_in_community", { name });
}

/** One member's card, or `null` when you may not see it. */
export function communityCard(memberId: string): Promise<CardView | null> {
  return call<CardView | null>("community_card", { memberId });
}

/**
 * One member's picture as standard base64 (a real PNG, at most 256 × 256, checked by Plenipo), or
 * `null` when there is none. Show it only as `data:image/png;base64,…`.
 */
export function communityPicture(memberId: string): Promise<string | null> {
  return call<string | null>("community_picture", { memberId });
}

/**
 * **Invite by email**: 8 West emails the address a link to join. The answer is the same whether or
 * not the address has an account. It needs Pro; if not, the error says so in plain words.
 */
export function inviteToCommunity(email: string): Promise<void> {
  return call<void>("invite_to_community", { email });
}

/** **Share my profile**: your share link and its picture code. Nothing is sent. */
export function shareMyCommunityProfile(): Promise<ShareProfile> {
  return call<ShareProfile>("share_my_community_profile");
}

// ---- Community → Messages: private messages, sealed (Phase 24, ADR-164, ADR-172, ADR-173) ----

/** Your conversations and the requests waiting for you, the most recent first. */
export function communityConversations(): Promise<ConversationSummary[]> {
  return call<ConversationSummary[]>("community_conversations");
}

/**
 * One conversation with one person: up to 100 messages and reactions (oldest first) before the
 * message `before` (its `itemId`; `null` for the newest). Opening it marks its messages seen.
 * `null` when this PC has no conversation with that person yet.
 */
export function communityConversation(
  memberId: string,
  before: string | null,
): Promise<ConversationView | null> {
  return call<ConversationView | null>("community_conversation", { memberId, before });
}

/**
 * **Send** a message to a member (`name` is their Community name, for a first message). `text` is
 * up to 4,000 characters; `replyTo` is the item ID of the message it answers. A first message to
 * someone you don't talk with is part of Pro: the error says so in plain words (kind `partOfPro`).
 */
export function sendCommunityMessage(
  to: string,
  name: string,
  text: string,
  replyTo: string | null,
): Promise<MessageView> {
  return call<MessageView>("send_community_message", { to, name, text, replyTo });
}

/**
 * React to a message with 👍 ❤️ 😂 😮 or 🙏; `null` takes your reaction back. Nothing else is
 * accepted.
 */
export function reactInCommunity(itemId: string, emoji: string | null): Promise<void> {
  return call<void>("react_in_community", { itemId, emoji });
}

/** **Accept** someone's first message (a request), so you can talk. */
export function acceptCommunityRequest(memberId: string): Promise<void> {
  return call<void>("accept_community_request", { memberId });
}

/**
 * **Leave this conversation**: it is deleted from this computer, and 8 West stops delivering the
 * other person's new messages (ADR-173). Your other computers keep their copies.
 */
export function leaveCommunityConversation(memberId: string): Promise<void> {
  return call<void>("leave_community_conversation", { memberId });
}

/** **Delete for me**: the message leaves this computer only. Nothing is sent (ADR-172). */
export function deleteCommunityMessage(itemId: string): Promise<void> {
  return call<void>("delete_community_message", { itemId });
}

/** **It matches**: you compared the safety code, so "computers changed" goes away. */
export function communitySafetyCodeChecked(memberId: string): Promise<void> {
  return call<void>("community_safety_code_checked", { memberId });
}

/**
 * Open a web address from a message in your own web browser (never Plenipo's). Call it only
 * after the person said yes to "Open this link in your web browser?". It refuses anything but a
 * plain `https://` or `http://` address.
 */
export function openCommunityLink(link: string): Promise<void> {
  return call<void>("open_community_link", { link });
}

/**
 * **Give to a worker**: the words of a message go to the full-time position `positionId` as an
 * objective, marked as outside words so the worker treats them as information, never as orders.
 * `note` is what you ask it to do with them (may be empty).
 */
export function giveCommunityMessageToWorker(
  itemId: string,
  positionId: string,
  note: string,
): Promise<AgentSessionDetail> {
  return call<AgentSessionDetail>("give_community_message_to_worker", {
    itemId,
    positionId,
    note,
  });
}

// ---- Community → Block, Report, and Delete my Community data (Phase 24, ADR-167, ADR-168) ----

/**
 * **Block** someone, by their ID and Community name. They can't message you, find your card, or
 * link with you, and they are not told. What is on this computer stays.
 */
export function blockInCommunity(memberId: string, name: string): Promise<void> {
  return call<void>("block_in_community", { memberId, name });
}

/** **Unblock** someone. Links that the block ended do not come back by themselves. */
export function unblockInCommunity(memberId: string): Promise<void> {
  return call<void>("unblock_in_community", { memberId });
}

/** The people you blocked, for Settings → Community → **Blocked**. */
export function communityBlocked(): Promise<BlockedPerson[]> {
  return call<BlockedPerson[]>("community_blocked");
}

/** **What is wrong?**, as a report sends it (the app's names; the screen has plain words). */
export type ReportReason =
  | "spam"
  | "harassment"
  | "scam"
  | "hate"
  | "sexual"
  | "under13"
  | "youngPersonRisk"
  | "impersonation"
  | "cheating"
  | "other";

/**
 * **Report** a person, their profile, or messages they sent you (1 to 20, each with its proof) to
 * 8 West. A report carries only what `of` names, the reason, and `note` (up to 1,000 characters).
 * `block` also blocks the person once the report is made.
 */
export function reportInCommunity(
  memberId: string,
  name: string,
  of: ReportOf,
  reason: ReportReason,
  note: string,
  block: boolean,
): Promise<void> {
  return call<void>("report_in_community", { memberId, name, of, reason, note, block });
}

/**
 * **Delete my Community data from this PC**: every Community conversation and message on this
 * computer. Nothing is sent; 8 West and the other people keep theirs. Ask first.
 */
export function deleteMyCommunityData(): Promise<void> {
  return call<void>("delete_my_community_data");
}

/**
 * **Your points**: the total, this week, your places, your badges, "Thanked by 12 people", and the
 * last 20 changes. It asks 8 West, which lets this and the leaderboard be asked 120 times an hour
 * together: ask once each time the page shows, never on a timer.
 */
export function communityPoints(): Promise<PointsView> {
  return call<PointsView>("community_points");
}

/**
 * **Leaderboard**: the top 50 of **This week** (`allTime` false) or **All time**, and your own
 * place. Members under 18 are not on it: do not ask for them. It asks 8 West: ask once each time
 * the page shows and when the person picks the other list, never on a timer.
 */
export function communityLeaderboard(allTime: boolean): Promise<LeaderboardView> {
  return call<LeaderboardView>("community_leaderboard", { allTime });
}

/** **Getting started**: which of its steps are done, and whether it was closed. Nothing is sent. */
export function communityGettingStarted(): Promise<GettingStarted> {
  return call<GettingStarted>("community_getting_started");
}

/** Close **Getting started** for good. It answers how things stand now. Nothing is sent. */
export function closeCommunityGettingStarted(): Promise<GettingStarted> {
  return call<GettingStarted>("close_community_getting_started");
}
