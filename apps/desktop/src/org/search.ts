import type { OrgSnapshot, PositionInfo } from "@plenipo/types";

import { runtimeLabel } from "./format";
import { ORG_ID, OWNER_ID, workerNodeId } from "./layout";

/** Lower-cased text a search matches for a position. */
export function positionSearchText(snapshot: OrgSnapshot, p: PositionInfo): string {
  const department = snapshot.departments.find((d) => d.id === p.departmentId)?.name ?? "";
  const project = snapshot.projects.find((x) => x.id === p.projectId)?.name ?? "";
  return `${p.title} ${p.roleName} ${department} ${project} ${runtimeLabel(snapshot, p.runtimeId)}`.toLowerCase();
}

/**
 * Canvas node IDs matching a search, in tree order (`null` when not searching): positions by
 * title, role, department, project, or runtime; live workers by objective.
 */
export function searchMatches(snapshot: OrgSnapshot, query: string): string[] | null {
  const q = query.trim().toLowerCase();
  if (q === "") return null;
  const out: string[] = [];
  if ("you owner".includes(q)) out.push(OWNER_ID);
  if (snapshot.name.toLowerCase().includes(q)) out.push(ORG_ID);
  for (const p of snapshot.positions) {
    if (!p.active) continue;
    if (positionSearchText(snapshot, p).includes(q)) out.push(p.id);
    for (const w of p.workers) {
      if (w.objective.toLowerCase().includes(q)) out.push(workerNodeId(w.agentId));
    }
  }
  return out;
}
