/**
 * The owner's control over workers (Phase 17), in plain words: experience (ADR-045), learning in
 * layers (ADR-041), and what is archived, deleted for good, or in your Workforce (ADR-043).
 */
import type {
  DepartmentInfo,
  ExperienceInfo,
  LearningInfo,
  OrgSnapshot,
  PositionInfo,
  ProjectInfo,
  SpecialtyInfo,
} from "@plenipo/types";

import { plural } from "./format";

/** "57 — 4 lessons you kept, 17 tasks done". */
export function experienceLine(e: ExperienceInfo): string {
  const reasons = [
    plural(e.keptLessons, "lesson you kept", "lessons you kept"),
    plural(e.tasksDone, "task done", "tasks done"),
  ];
  return `${e.score} — ${reasons.join(", ")}`;
}

/** Whether it learns, and which setting decided, in one sentence. */
export function learningLine(info: LearningInfo, roleName: string): string {
  if (info.from === "organization") {
    return "It does not learn: Worker learning is off for everyone (Settings → Switches).";
  }
  const does = info.learns ? "It learns from its work" : "It does not learn";
  return info.from === "agent"
    ? `${does}: its own setting.`
    : `${does}: the ${roleName} role's setting.`;
}

/** On the chart. */
export const onChart = (p: PositionInfo): boolean => p.active;

/** Archived, and still there to bring back or delete (not deleted, not in the Workforce). */
export const isArchived = (p: PositionInfo): boolean => !p.active && !p.deleted && !p.inWorkforce;

/** A department on the chart (not archived, not deleted). */
export const liveDepartment = (d: DepartmentInfo): boolean => !d.deleted && d.archivedAt === null;

/** A project on the chart (not archived, not deleted). */
export const liveProject = (p: ProjectInfo): boolean => !p.deleted && p.active;

/** An archived department, still there to bring back or delete. */
export const archivedDepartment = (d: DepartmentInfo): boolean =>
  !d.deleted && d.archivedAt !== null;

/** An archived project, still there to bring back or delete. */
export const archivedProject = (p: ProjectInfo): boolean => !p.deleted && !p.active;

/** "with the Website project", or "on its own". */
export function archivedWithLine(p: { archivedWith: PositionInfo["archivedWith"] }): string {
  return p.archivedWith ? `with the ${p.archivedWith.name} ${p.archivedWith.kind}` : "on its own";
}

/** The role a "Hire a VP" button starts with: the built-in VP role, else the first of yours. */
export function vpRoleId(snapshot: OrgSnapshot): string | null {
  const vps = snapshot.roles.filter((r) => r.kind === "superintendent");
  return (vps.find((r) => r.template) ?? vps[0])?.id ?? null;
}

/** The position's role's specialties (built-in first, then yours). */
export function specialtiesOf(snapshot: OrgSnapshot, roleId: string): SpecialtyInfo[] {
  const role = snapshot.roles.find((r) => r.id === roleId);
  if (!role) return [];
  return [...role.specialties].sort(
    (a, b) => Number(b.builtIn) - Number(a.builtIn) || a.name.localeCompare(b.name),
  );
}

/** Everything archived, for the Archived tab: departments, projects, and agents. */
export function archivedItems(snapshot: OrgSnapshot) {
  return {
    departments: snapshot.departments.filter(archivedDepartment),
    projects: snapshot.projects.filter(archivedProject),
    positions: snapshot.positions.filter(isArchived),
  };
}

/** How many archived items there are, for the tab's label. */
export function archivedCount(snapshot: OrgSnapshot): number {
  const a = archivedItems(snapshot);
  return a.departments.length + a.projects.length + a.positions.length;
}
