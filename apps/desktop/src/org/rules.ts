/**
 * The organization's structure rules, as hints for the canvas: which drops to highlight and
 * which options to offer. The Ledger enforces the rules (in the same transaction as the change)
 * and its refusal is what the owner sees when these hints and the Ledger ever disagree.
 */
import type { OrgSnapshot, OversightRole, PositionInfo, RoleInfo } from "@plenipo/types";

import { OVERSIGHT_NOUN } from "./format";
import { rankName, titlesOf, withArticle, type TitleSet } from "./titles";

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

/** "A Supervisor", "An Underboss". */
function aRank(t: TitleSet, kind: Kind): string {
  const phrase = withArticle(rankName(t, kind));
  return phrase.charAt(0).toUpperCase() + phrase.slice(1);
}

/** Why a position of `kind` cannot report to `to` (`null`: the owner); `null` when it can. */
function supervisorRefusal(t: TitleSet, kind: Kind, to: PositionInfo | null): string | null {
  const supervisor = `${aRank(t, "projectCoordinator")} reports to the ${rankName(t, "departmentManager")} of a department.`;
  if (to === null) {
    switch (kind) {
      case "projectCoordinator":
        return supervisor;
      case "worker":
        return `${aRank(t, "worker")} reports to a full-time position on a team, such as ${withArticle(rankName(t, "projectCoordinator"))}, not directly to you.`;
      default:
        return null;
    }
  }
  if (!to.active) return `${to.title} has been archived.`;
  if (to.staffing !== "persistent") {
    return `${to.title} is an on-call position; only full-time positions lead a team.`;
  }
  if ((kind === "superintendent" || kind === "departmentManager") && to.kind !== "superintendent") {
    return `${aRank(t, kind)} reports to you or to ${withArticle(rankName(t, "superintendent"))}.`;
  }
  if (kind === "projectCoordinator" && !to.headsDepartmentId) {
    return supervisor;
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
  if (position.loan) return lentAway(position);
  if (to === position.reportsTo) {
    return to === null ? "Already reports to you." : "Already reports there.";
  }
  const t = titlesOf(snapshot);
  if (to === null) return supervisorRefusal(t, position.kind, null);
  const byId = positionMap(snapshot);
  const target = byId.get(to);
  if (!target) return "That position no longer exists.";
  if (to === position.id) return "A position cannot report to itself.";
  if (isWithin(byId, to, position.id)) {
    return `${target.title} reports to ${position.title}, so ${position.title} cannot report to it.`;
  }
  return supervisorRefusal(t, position.kind, target);
}

/** Roles that can be hired directly (managers and supervisors come with their department or project). */
export function hireableRoles(snapshot: OrgSnapshot): RoleInfo[] {
  return snapshot.roles.filter((r) => r.kind === "superintendent" || r.kind === "worker");
}

/** Why a new position of `role` cannot report to `to` (`null`: the owner); `null` when it can. */
export function hireRefusal(
  snapshot: OrgSnapshot,
  role: RoleInfo,
  to: string | null,
): string | null {
  const t = titlesOf(snapshot);
  if (role.kind === "departmentManager") {
    return `${aRank(t, "departmentManager")} comes with a department: create a department instead.`;
  }
  if (role.kind === "projectCoordinator") {
    return `${aRank(t, "projectCoordinator")} comes with a project: create a project instead.`;
  }
  if (to === null) return supervisorRefusal(t, role.kind, null);
  const target = positionMap(snapshot).get(to);
  if (!target) return "That position no longer exists.";
  return supervisorRefusal(t, role.kind, target);
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
  if (overseer.loan) return lentAway(overseer);
  if (overseer.staffing === "persistent") {
    return `${overseer.title} is a full-time position; for now only on-call positions review, QA, or audit a team.`;
  }
  if (target.staffing !== "persistent") {
    return `${target.title} is an on-call position and has no team to oversee.`;
  }
  if (overseer.reportsTo === target.id) {
    return `${overseer.title} is already on ${target.title}'s team.`;
  }
  const existing = snapshot.oversight.some(
    (o) => o.overseerId === overseer.id && o.targetId === target.id && o.role === role,
  );
  if (existing) {
    return `${overseer.title} is already the ${OVERSIGHT_NOUN[role]} for ${target.title}'s team.`;
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

/** Positions that can be given an objective: full-time, staffed, and active. */
export function canTakeObjective(p: PositionInfo): boolean {
  return p.active && p.staffing === "persistent" && p.agent !== null;
}

/** The AI tool a new position should start with: the first ready one the project allows. */
export function defaultRuntime(snapshot: OrgSnapshot, allowed: string[] | null): string {
  const pool = snapshot.runtimes.filter((r) => allowed === null || allowed.includes(r.id));
  return (pool.find((r) => r.ready) ?? pool[0] ?? snapshot.runtimes[0])?.id ?? "";
}

/** "Security Auditor is lent to Shop Supervisor's team; send it home first." */
function lentAway(p: PositionInfo): string {
  return `${p.title} is lent to ${p.loan?.to ?? "another"}'s team; send it home first.`;
}

/**
 * Why `position` cannot be lent to `lead`'s team (ADR-054 §2–§3); `null` when it can. The
 * Ledger also checks that nothing it has is unfinished, its title, and the AI tools there.
 */
export function lendRefusal(
  snapshot: OrgSnapshot,
  position: PositionInfo,
  lead: PositionInfo,
): string | null {
  if (!position.active) return `${position.title} has been archived.`;
  if (position.staffing === "persistent") {
    return `${position.title} is a full-time position: move it instead of lending it.`;
  }
  if (position.loan) return `${position.title} is already lent; send it home first.`;
  if (!lead.active) return `${lead.title} has been archived.`;
  if (lead.staffing !== "persistent") {
    return `${lead.title} is an on-call position and has no team to lend to.`;
  }
  if (position.reportsTo === lead.id)
    return `${position.title} is already on ${lead.title}'s team.`;
  if (snapshot.oversight.some((o) => o.overseerId === position.id && o.targetId === lead.id)) {
    return `${position.title} already oversees ${lead.title}'s team.`;
  }
  return null;
}

/** What dropping `position` on the trash can archives (ADR-053 §9–§10). */
export type TrashTarget =
  | { kind: "position"; id: string }
  | { kind: "project"; id: string; name: string }
  | { kind: "department"; id: string; name: string };

/** What the trash can would archive for `position`, or why it cannot. */
export function trashTarget(
  snapshot: OrgSnapshot,
  position: PositionInfo,
): TrashTarget | { refused: string } {
  if (!position.active) return { refused: `${position.title} has been archived.` };
  if (position.loan) return { refused: lentAway(position) };
  if (position.coordinatesProjectId) {
    const project = snapshot.projects.find((p) => p.id === position.coordinatesProjectId);
    if (project) return { kind: "project", id: project.id, name: project.name };
  }
  if (position.headsDepartmentId) {
    const department = snapshot.departments.find((d) => d.id === position.headsDepartmentId);
    if (department) return { kind: "department", id: department.id, name: department.name };
  }
  if (snapshot.positions.some((p) => p.active && p.reportsTo === position.id)) {
    const t = titlesOf(snapshot);
    return {
      refused:
        position.kind === "superintendent"
          ? `${position.title} leads departments: archive each department first (drop its ${rankName(t, "departmentManager")} here).`
          : `${position.title} leads a team: move or archive its team first.`,
    };
  }
  return { kind: "position", id: position.id };
}
