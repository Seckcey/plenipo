// Organization DTO fixtures for tests (and the visual preview harness).
import type {
  DepartmentInfo,
  OrgSnapshot,
  OrgStats,
  OversightInfo,
  PositionInfo,
  PositionKind,
  ProjectInfo,
  RoleInfo,
  Staffing,
  WorkerInfo,
} from "@plenipo/types";

const T0 = Date.UTC(2026, 8, 26, 15, 0, 0);

export const role = (
  id: string,
  name: string,
  kind: PositionKind,
  staffing: Staffing,
  glyph: string,
): RoleInfo => ({
  id,
  name,
  description: `${name} role.`,
  kind,
  staffing,
  template: true,
  glyph,
  purpose: [],
  defaultCapabilities: [],
});

export const ROLES: RoleInfo[] = [
  role("r-super", "Superintendent", "superintendent", "persistent", "executive"),
  role("r-manager", "Department Manager", "departmentManager", "persistent", "manager"),
  role("r-coord", "Project Coordinator", "projectCoordinator", "persistent", "coordinator"),
  role("r-dev", "Senior Developer", "worker", "onDemand", "code"),
  role("r-review", "Code Reviewer", "worker", "onDemand", "review"),
  role("r-qa", "QA Engineer", "worker", "onDemand", "qa"),
  role("r-sec", "Security Auditor", "worker", "onDemand", "shield"),
  role("r-docs", "Documentation Writer", "worker", "onDemand", "docs"),
  role("r-research", "Researcher", "worker", "onDemand", "research"),
  role("r-design", "Designer", "worker", "onDemand", "design"),
];

const ROLE_BY_ID = new Map(ROLES.map((r) => [r.id, r]));

export const worker = (agentId: string, patch: Partial<WorkerInfo> = {}): WorkerInfo => ({
  agentId,
  taskId: `task-${agentId}`,
  objective: `Work of ${agentId}`,
  state: "running",
  sessionId: `session-${agentId}`,
  runtimeId: "claude-code",
  model: null,
  parentTaskId: null,
  spawnedAt: T0 - 5 * 60_000,
  startedAt: T0 - 4 * 60_000,
  ...patch,
});

export const position = (
  id: string,
  title: string,
  roleId: string,
  reportsTo: string | null,
  patch: Partial<PositionInfo> = {},
): PositionInfo => {
  const r = ROLE_BY_ID.get(roleId) ?? ROLES[3]!;
  const persistent = r.staffing === "persistent";
  return {
    id,
    title,
    roleId,
    roleName: r.name,
    kind: r.kind,
    staffing: r.staffing,
    reportsTo,
    departmentId: null,
    projectId: null,
    headsDepartmentId: null,
    coordinatesProjectId: null,
    runtimeId: "claude-code",
    model: null,
    active: true,
    sortKey: 0,
    agent: persistent
      ? {
          id: `agent-${id}`,
          runtimeId: "claude-code",
          model: null,
          sessionId: null,
          hiredAt: T0 - 3_600_000,
        }
      : null,
    workers: [],
    status: "idle",
    statusDetail: null,
    currentTask: null,
    counts: { working: 0, waiting: 0, queued: 0 },
    history: { retired: 0, failed: 0, lastRetiredAt: null },
    createdAt: T0 - 7_200_000,
    archivedAt: null,
    ...patch,
  };
};

export const department = (
  id: string,
  name: string,
  head: string,
  projectIds: string[],
): DepartmentInfo => ({
  id,
  name,
  description: `${name} department.`,
  active: true,
  headPositionId: head,
  projectIds,
  createdAt: T0 - 7_000_000,
});

export const project = (
  id: string,
  name: string,
  departmentId: string,
  coordinator: string,
  allowedRuntimes: string[] = ["claude-code", "codex"],
): ProjectInfo => ({
  id,
  name,
  description: `${name} project.`,
  departmentId,
  repositoryUrl: null,
  localPath: null,
  allowedRuntimes,
  capabilityProfile: null,
  coordinatorPositionId: coordinator,
  active: true,
  createdAt: T0 - 6_900_000,
});

function stats(positions: PositionInfo[], departments: number, projects: number): OrgStats {
  const active = positions.filter((p) => p.active);
  const persistent = active.filter((p) => p.staffing === "persistent");
  const workers = active.flatMap((p) => p.workers);
  return {
    departments,
    projects,
    positions: active.length,
    staffed: persistent.filter((p) => p.agent).length,
    vacant: persistent.filter((p) => !p.agent).length,
    activeWorkers: workers.length,
    working:
      workers.filter((w) => w.state === "running").length +
      persistent.filter((p) => p.status === "working").length,
    waiting:
      workers.filter((w) => w.state === "blocked").length +
      persistent.filter((p) => p.status === "waiting").length,
    queued: workers.filter((w) => w.state === "queued").length,
    completed24h: 7,
    failed24h: 1,
  };
}

/** An organization with no positions yet (as on first start). */
export function emptyOrganization(): OrgSnapshot {
  return {
    name: "Organization",
    titles: "business",
    roles: ROLES,
    departments: [],
    projects: [],
    positions: [],
    oversight: [],
    stats: stats([], 0, 0),
    runtimes: [
      { id: "claude-code", label: "Claude Code", ready: true },
      { id: "codex", label: "Codex", ready: false },
    ],
    notices: [],
    generatedAt: T0,
  };
}

/**
 * Two departments under a superintendent; Engineering runs Website Relaunch with a working team
 * (a security auditor oversees it), Marketing runs Q4 Campaign (the QA engineer also QAs it).
 */
export function sampleOrganization(): OrgSnapshot {
  const eng = { departmentId: "d-eng" };
  const web = { ...eng, projectId: "pr-web" };
  const mkt = { departmentId: "d-mkt" };
  const camp = { ...mkt, projectId: "pr-camp" };
  const positions: PositionInfo[] = [
    position("p-super", "Superintendent", "r-super", null, {
      status: "working",
      counts: { working: 1, waiting: 0, queued: 0 },
      currentTask: {
        id: "task-super",
        objective: "Relaunch the website before the Q4 campaign",
        state: "running",
        positionId: "p-super",
        positionTitle: "Superintendent",
        projectId: null,
        parentTaskId: null,
        sessionId: "session-super",
        createdAt: T0 - 600_000,
        startedAt: T0 - 590_000,
        completedAt: null,
      },
    }),
    position("p-eng", "Engineering Manager", "r-manager", "p-super", {
      ...eng,
      headsDepartmentId: "d-eng",
    }),
    position("p-web", "Website Coordinator", "r-coord", "p-eng", {
      ...web,
      coordinatesProjectId: "pr-web",
      status: "waiting",
      counts: { working: 0, waiting: 1, queued: 0 },
    }),
    position("p-dev", "Senior Developer", "r-dev", "p-web", {
      ...web,
      status: "working",
      counts: { working: 1, waiting: 0, queued: 1 },
      workers: [
        worker("a-dev-1", { objective: "Build the new pricing page", parentTaskId: "task-web" }),
        worker("a-dev-2", {
          objective: "Migrate the blog to the new theme",
          state: "queued",
          startedAt: null,
          sessionId: null,
        }),
      ],
    }),
    position("p-review", "Code Reviewer", "r-review", "p-web", web),
    position("p-qa", "QA Engineer", "r-qa", "p-web", {
      ...web,
      status: "working",
      counts: { working: 1, waiting: 0, queued: 0 },
      workers: [worker("a-qa-1", { objective: "Verify checkout on mobile", runtimeId: "codex" })],
    }),
    position("p-sec", "Security Auditor", "r-sec", "p-eng", eng),
    position("p-mkt", "Marketing Manager", "r-manager", "p-super", {
      ...mkt,
      headsDepartmentId: "d-mkt",
      agent: null,
      status: "vacant",
    }),
    position("p-camp", "Campaign Coordinator", "r-coord", "p-mkt", {
      ...camp,
      coordinatesProjectId: "pr-camp",
    }),
    position("p-design", "Designer", "r-design", "p-camp", {
      ...camp,
      runtimeId: "codex",
      status: "unavailable",
      statusDetail: "Codex is not signed in",
    }),
    position("p-docs", "Documentation Writer", "r-docs", "p-camp", camp),
  ];
  const oversight: OversightInfo[] = [
    { id: "o-sec", role: "security", overseerId: "p-sec", targetId: "p-web", createdAt: T0 },
    { id: "o-qa", role: "qa", overseerId: "p-qa", targetId: "p-camp", createdAt: T0 },
  ];
  return {
    name: "Northwind Studio",
    titles: "business",
    roles: ROLES,
    departments: [
      department("d-eng", "Engineering", "p-eng", ["pr-web"]),
      department("d-mkt", "Marketing", "p-mkt", ["pr-camp"]),
    ],
    projects: [
      project("pr-web", "Website Relaunch", "d-eng", "p-web"),
      project("pr-camp", "Q4 Campaign", "d-mkt", "p-camp", ["claude-code"]),
    ],
    positions,
    oversight,
    stats: stats(positions, 2, 2),
    runtimes: [
      { id: "claude-code", label: "Claude Code", ready: true },
      { id: "codex", label: "Codex", ready: false },
    ],
    notices: [],
    generatedAt: T0,
  };
}
