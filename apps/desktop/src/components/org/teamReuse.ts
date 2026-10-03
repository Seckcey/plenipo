import type { OrgSnapshot, PositionInfo } from "@plenipo/types";

/**
 * For each job of a new project's team, the worker already in `departmentId` it would use
 * (Phase 25, item 2.7): an active on-call worker with the job's role, not lent away, each used
 * once. `null`: none, so one is hired.
 */
export function reusableWorkers(
  snapshot: OrgSnapshot,
  departmentId: string | null,
  jobs: readonly string[],
): (PositionInfo | null)[] {
  const used = new Set<string>();
  return jobs.map((role) => {
    const p = snapshot.positions.find(
      (x) =>
        departmentId !== null &&
        x.active &&
        x.departmentId === departmentId &&
        x.roleName === role &&
        x.kind === "worker" &&
        x.staffing !== "persistent" &&
        !x.loan &&
        !used.has(x.id),
    );
    if (!p) return null;
    used.add(p.id);
    return p;
  });
}
