/**
 * The organization's structure rules, as hints for the canvas: which drops to highlight and
 * which options to offer. The Ledger enforces the rules (in the same transaction as the change)
 * and its refusal is what the owner sees when these hints and the Ledger ever disagree.
 */
import type { OrgSnapshot, OversightRole, PositionInfo, RoleInfo } from "@plenipo/types";

import { OVERSIGHT_LABEL } from "./format";

export function positionMap(snapshot: OrgSnapshot): Map<string, PositionInfo> {
  return new Map(snapshot.positions.map((p) => [p.id, p]));
}

/** `candidate` is `root` or reports to it, directly or through others. */
export function isWithin(
  byId: Map<string, PositionInfo>,
  candidate: string,
  root: string,
): boolean {
  const seen = new Set<string>();
  let current: string | null = candidate;
  while (current && !seen.has(current)) {
    if (current === root) return true;
    seen.add(current);
    current = byId.get(current)?.reportsTo ?? null;
  }
  return false;
}

type Kind = PositionInfo["kind"];

/** Why a position of `kind` cannot report to `to` (`null`: the owner); `null` when it can. */
function supervisorRefusal(kind: Kind, to: PositionInfo | null): string | null {
  if (to === null) {
    switch (kind) {
      case "projectCoordinator":
        return "A project coordinator reports to the head of a department.";
      case "worker":
        return "A worker reports to a manager, coordinator, or other persistent position, not directly to you.";
      default:
        return null;
    }
  }
  if (!to.active) return `${to.title} has been archived.`;
  if (to.staffing !== "persistent") {
    return `${to.title} is an on-demand position; only persistent positions lead a team.`;
  }
  if ((kind === "superintendent" || kind === "departmentManager") && to.kind !== "superintendent") {
    return "A superintendent or department manager reports to you or to a superintendent.";
  }
  if (kind === "projectCoordinator" && !to.headsDepartmentId) {
    return "A project coordinator reports to the head of a department.";
  }
  return null;
}

/** Why `position` cannot move to report to `to` (`null`: the owner); `null` when it can. */
export function moveRefusal(
  snapshot: OrgSnapshot,
  position: PositionInfo,
  to: string | null,
): string | null {
  if (!position.active) return `${position.title} has been archived.`;
  if (to === position.reportsTo) {
    return to === null ? "Already reports to you." : "Already reports there.";
  }
  if (to === null) return supervisorRefusal(position.kind, null);
  const byId = positionMap(snapshot);
  const target = byId.get(to);
  if (!target) return "That position no longer exists.";
  if (to === position.id) return "A position cannot report to itself.";
  if (isWithin(byId, to, position.id)) {
    return `${target.title} reports to ${position.title}, so ${position.title} cannot report to it.`;
  }
  return supervisorRefusal(position.kind, target);
}

/** Roles that can be hired directly (managers and coordinators come with their department or project). */
export function hireableRoles(snapshot: OrgSnapshot): RoleInfo[] {
  return snapshot.roles.filter((r) => r.kind === "superintendent" || r.kind === "worker");
}

/** Why a new position of `role` cannot report to `to` (`null`: the owner); `null` when it can. */
export function hireRefusal(
  snapshot: OrgSnapshot,
  role: RoleInfo,
  to: string | null,
): string | null {
  if (role.kind === "departmentManager") {
    return "A department manager comes with its department: create a department instead.";
  }
  if (role.kind === "projectCoordinator") {
    return "A project coordinator comes with its project: create a project instead.";
  }
  if (to === null) return supervisorRefusal(role.kind, null);
  const target = positionMap(snapshot).get(to);
  if (!target) return "That position no longer exists.";
  return supervisorRefusal(role.kind, target);
}

/** Supervisors a new position of `role` may report to; `null` is the owner. */
export function supervisorChoices(snapshot: OrgSnapshot, role: RoleInfo): (string | null)[] {
  const choices: (string | null)[] = [];
  if (hireRefusal(snapshot, role, null) === null) choices.push(null);
  for (const p of snapshot.positions) {
    if (p.active && hireRefusal(snapshot, role, p.id) === null) choices.push(p.id);
  }
  return choices;
}

/** Positions `position` may move under; `null` is the owner. */
export function moveChoices(snapshot: OrgSnapshot, position: PositionInfo): (string | null)[] {
  const choices: (string | null)[] = [];
  if (moveRefusal(snapshot, position, null) === null) choices.push(null);
  for (const p of snapshot.positions) {
    if (p.active && moveRefusal(snapshot, position, p.id) === null) choices.push(p.id);
  }
  return choices;
}

/** Why `overseer` cannot oversee `target`'s team as `role`; `null` when it can. */
export function oversightRefusal(
  snapshot: OrgSnapshot,
  overseer: PositionInfo,
  target: PositionInfo,
  role: OversightRole,
): string | null {
  if (overseer.id === target.id) return "A position cannot oversee its own team.";
  if (!overseer.active || !target.active)
    return "Archived positions cannot oversee or be overseen.";
  if (overseer.staffing === "persistent") {
    return `${overseer.title} is a persistent position; in this phase only on-demand positions review, QA, or audit a team.`;
  }
  if (target.staffing !== "persistent") {
    return `${target.title} is an on-demand position and has no team to oversee.`;
  }
  if (overseer.reportsTo === target.id) {
    return `${overseer.title} is already on ${target.title}'s team.`;
  }
  const existing = snapshot.oversight.some(
    (o) => o.overseerId === overseer.id && o.targetId === target.id && o.role === role,
  );
  if (existing) {
    return `${overseer.title} is already the ${OVERSIGHT_LABEL[role].toLowerCase()} for ${target.title}'s team.`;
  }
  return null;
}

/** Oversight roles, the one that suits this position's role first. */
export function oversightOrder(glyph: string): OversightRole[] {
  const first: OversightRole | null =
    glyph === "qa" ? "qa" : glyph === "shield" ? "security" : glyph === "review" ? "review" : null;
  const all: OversightRole[] = ["review", "qa", "security"];
  return first ? [first, ...all.filter((r) => r !== first)] : all;
}

/** Positions that can be given an objective: persistent, staffed, and active. */
export function canTakeObjective(p: PositionInfo): boolean {
  return p.active && p.staffing === "persistent" && p.agent !== null;
}

/** The runtime a new position should start with: the first ready one the project allows. */
export function defaultRuntime(snapshot: OrgSnapshot, allowed: string[] | null): string {
  const pool = snapshot.runtimes.filter((r) => allowed === null || allowed.includes(r.id));
  return (pool.find((r) => r.ready) ?? pool[0] ?? snapshot.runtimes[0])?.id ?? "";
}
