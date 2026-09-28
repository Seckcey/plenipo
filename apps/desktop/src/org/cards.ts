import type { ActivityScope, OrgSnapshot, PositionInfo, PositionStatus } from "@plenipo/types";
import type { EntityCardProps, Status } from "@plenipo/ui";

import { STATUS_LABEL } from "./format";
import { rankName, titlesOf } from "./titles";

/** How a position's state reads on the design system's status ramp. */
export const POSITION_STATUS: Record<PositionStatus, Status> = {
  working: "ok",
  waiting: "pending",
  queued: "pending",
  idle: "offline",
  vacant: "offline",
  unavailable: "warn",
  archived: "offline",
};

function lead(org: OrgSnapshot, id: string | null): PositionInfo | undefined {
  return id ? org.positions.find((p) => p.id === id) : undefined;
}

function runtimeLabel(org: OrgSnapshot, p: PositionInfo | undefined): string {
  if (!p) return "not filled";
  return org.runtimes.find((r) => r.id === p.runtimeId)?.label ?? "Automatic";
}

/** A card for each active department and project, with its scope for the activity query. */
export function organizationCards(
  org: OrgSnapshot,
): { scope: ActivityScope; card: EntityCardProps }[] {
  const titles = titlesOf(org);
  const departments = org.departments
    .filter((d) => d.active)
    .map((d) => {
      const head = lead(org, d.headPositionId);
      // Projects on the chart (not archived or deleted for good), as on the Home page.
      const projects = d.projectIds.filter(
        (id) => org.projects.find((p) => p.id === id)?.active,
      ).length;
      return {
        scope: { kind: "department", id: d.id } satisfies ActivityScope,
        card: {
          title: d.name,
          status: head ? POSITION_STATUS[head.status] : "offline",
          statusLabel: head
            ? STATUS_LABEL[head.status]
            : `No ${rankName(titles, "departmentManager")} yet`,
          subtype: "Department",
          owner: {
            icon: "user",
            label: `${rankName(titles, "departmentManager")} · ${runtimeLabel(org, head)}`,
          },
          resources: [],
          footerNote:
            projects === 0
              ? "No projects yet"
              : `${projects} ${projects === 1 ? "project" : "projects"}`,
        } satisfies EntityCardProps,
      };
    });
  const projects = org.projects
    .filter((p) => p.active)
    .map((p) => {
      const supervisor = lead(org, p.coordinatorPositionId);
      const department = org.departments.find((d) => d.id === p.departmentId);
      return {
        scope: { kind: "project", id: p.id } satisfies ActivityScope,
        card: {
          title: p.name,
          status: supervisor ? POSITION_STATUS[supervisor.status] : "offline",
          statusLabel: supervisor
            ? STATUS_LABEL[supervisor.status]
            : `No ${rankName(titles, "projectCoordinator")} yet`,
          subtype: `Project${department ? ` · ${department.name}` : ""}`,
          owner: {
            icon: "user",
            label: `${rankName(titles, "projectCoordinator")} · ${runtimeLabel(org, supervisor)}`,
          },
          resources: [
            ...(p.localPath ? [{ icon: "file" as const, label: "Project folder" }] : []),
            ...(p.repositoryUrl ? [{ icon: "branch" as const, label: "Git repository" }] : []),
            ...(p.allowedRuntimes.length > 0
              ? [
                  {
                    icon: "aiTools" as const,
                    label: `${p.allowedRuntimes.length} AI ${p.allowedRuntimes.length === 1 ? "tool" : "tools"} allowed`,
                  },
                ]
              : []),
          ],
        } satisfies EntityCardProps,
      };
    });
  return [...departments, ...projects];
}
