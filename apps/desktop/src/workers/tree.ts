/**
 * The Workers page's tree (I4): the organization shown in the top bar, as the owner thinks of
 * it. Each department with its VP, then its projects, each with its Supervisor and team, then the
 * department's own staff; an on-call position's workers working now under it. The President's
 * own staff (reporting straight to you) come first. Plain functions, so they are easy to test.
 */
import type { OrgSnapshot, PositionInfo, PositionStatus, WorkerInfo } from "@plenipo/types";

export type TreeRow = GroupRow | AgentRow;

/** A department or a project: it opens and closes. */
export interface GroupRow {
  kind: "department" | "project";
  /** `department:<id>` or `project:<id>`, stable across reloads (what stays open is kept). */
  key: string;
  id: string;
  name: string;
  children: TreeRow[];
}

/** A position: selecting it shows its agent's chat. */
export interface AgentRow {
  kind: "agent";
  /** `position:<id>`, the same key as its chat. */
  key: string;
  position: PositionInfo;
  /** An on-call position's workers working now, each in a conversation of its own. */
  workers: WorkerInfo[];
}

/** Doing something now: working, waiting for its team, or waiting for a free slot. */
export function isBusyStatus(status: PositionStatus): boolean {
  return status === "working" || status === "waiting" || status === "queued";
}

function byOrder(a: PositionInfo, b: PositionInfo): number {
  return a.sortKey - b.sortKey || a.title.localeCompare(b.title);
}

function agentRow(p: PositionInfo): AgentRow {
  return {
    kind: "agent",
    key: `position:${p.id}`,
    position: p,
    workers: p.workers.filter((w) => w.sessionId !== null),
  };
}

/** The organization as a tree; only what is part of it now (nothing archived or deleted). */
export function buildTree(org: OrgSnapshot): TreeRow[] {
  const positions = org.positions.filter((p) => p.active && !p.deleted).sort(byOrder);
  const used = new Set<string>();
  const take = (p: PositionInfo | undefined): AgentRow[] => {
    if (!p || used.has(p.id)) return [];
    used.add(p.id);
    return [agentRow(p)];
  };
  const byId = new Map(positions.map((p) => [p.id, p]));
  const activeProject = new Set(org.projects.filter((x) => x.active).map((x) => x.id));

  const projectRow = (projectId: string): GroupRow | null => {
    const project = org.projects.find((x) => x.id === projectId && x.active);
    if (!project) return null;
    const lead = project.coordinatorPositionId
      ? byId.get(project.coordinatorPositionId)
      : undefined;
    const team = positions.filter((p) => p.projectId === project.id && p.id !== lead?.id);
    return {
      kind: "project",
      key: `project:${project.id}`,
      id: project.id,
      name: project.name,
      children: [...take(lead), ...team.flatMap((p) => take(p))],
    };
  };

  const rows: TreeRow[] = [];
  // Your own staff: reporting to you, outside every department.
  for (const p of positions) {
    if (p.departmentId === null && p.projectId === null && p.reportsTo === null) {
      rows.push(...take(p));
    }
  }
  for (const d of org.departments.filter((x) => x.active && !x.deleted)) {
    const head = d.headPositionId ? byId.get(d.headPositionId) : undefined;
    const children: TreeRow[] = [...take(head)];
    for (const project of org.projects.filter((x) => x.active && x.departmentId === d.id)) {
      const row = projectRow(project.id);
      if (row) children.push(row);
    }
    // The department's own staff, outside its projects (or in one no longer active).
    for (const p of positions) {
      if (p.departmentId === d.id && !activeProject.has(p.projectId ?? "")) {
        children.push(...take(p));
      }
    }
    rows.push({ kind: "department", key: `department:${d.id}`, id: d.id, name: d.name, children });
  }
  // Projects outside any department.
  for (const project of org.projects.filter((x) => x.active && x.departmentId === null)) {
    const row = projectRow(project.id);
    if (row) rows.push(row);
  }
  // Anyone left over (a position whose department or project is gone): still shown.
  for (const p of positions) rows.push(...take(p));
  return rows;
}

/** Every agent in `rows`, at any depth. */
export function agentsIn(rows: readonly TreeRow[]): AgentRow[] {
  return rows.flatMap((r) => (r.kind === "agent" ? [r] : agentsIn(r.children)));
}

/** How many under a group are doing something now ("2 working" on a closed row). */
export function busyCount(row: GroupRow): number {
  return agentsIn(row.children).filter((a) => isBusyStatus(a.position.status)).length;
}

/**
 * What the page shows: a position's chat, one conversation's chat (another page's link, a
 * worker's own task, or one outside the organization), or the form to start one. Kept as one
 * string with the app's other places: `position:<id>`, `start`, or a conversation's ID.
 */
export type Selection =
  { kind: "position"; id: string } | { kind: "session"; id: string } | { kind: "start" } | null;

export const START = "start";

export function readSelection(id: string | null): Selection {
  if (!id) return null;
  if (id === START) return { kind: "start" };
  if (id.startsWith("position:")) return { kind: "position", id: id.slice("position:".length) };
  return { kind: "session", id };
}

/** The groups that hold `positionId`, outermost first: opened to show it when it is selected. */
export function pathTo(rows: readonly TreeRow[], positionId: string): string[] {
  for (const r of rows) {
    if (r.kind === "agent") {
      if (r.position.id === positionId) return [];
      continue;
    }
    const inner = pathTo(r.children, positionId);
    if (inner.length > 0 || agentsIn(r.children).some((a) => a.position.id === positionId)) {
      return [r.key, ...inner];
    }
  }
  return [];
}
