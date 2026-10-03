/**
 * The AI companies never used for some work, and where each entry comes from (Phase 25, item 1.6;
 * ADR-191): the whole organization's list (where lists are set since item 2.6), and any list kept
 * from before on its department, its role, or the agent itself. Shown next to every model picker,
 * so it is plain why a model is never chosen.
 */
import type { RoutingSnapshot } from "@plenipo/types";

export interface NeverUsed {
  /** The company, as shown ("DeepSeek"). */
  company: string;
  /** Where the entry comes from ("the whole organization"). */
  from: string;
}

/** Every entry that applies to work at `at`, closest last, each company once (its first source). */
export function neverUsed(
  snapshot: RoutingSnapshot,
  at: { departmentId?: string | null; roleId?: string | null; positionId?: string | null },
): NeverUsed[] {
  const label = (id: string) => snapshot.companies.find((c) => c.id === id)?.label ?? id;
  const layers: { from: string; ids: string[] }[] = [
    { from: "the whole organization", ids: snapshot.organization.neverCompanies },
  ];
  const department = at.departmentId
    ? snapshot.departments.find((d) => d.departmentId === at.departmentId)
    : undefined;
  if (department) {
    layers.push({ from: `the ${department.name} department`, ids: department.rule.neverCompanies });
  }
  const role = at.roleId ? snapshot.roles.find((r) => r.roleId === at.roleId) : undefined;
  if (role) layers.push({ from: `${role.roleName}'s choices`, ids: role.policy.neverCompanies });
  const agent = at.positionId
    ? snapshot.agents.find((a) => a.positionId === at.positionId)
    : undefined;
  if (agent) layers.push({ from: `${agent.title}'s own rule`, ids: agent.rule.neverCompanies });
  const out: NeverUsed[] = [];
  for (const layer of layers) {
    for (const id of layer.ids) {
      const company = label(id);
      if (!out.some((n) => n.company === company)) out.push({ company, from: layer.from });
    }
  }
  return out;
}

/** "Never used here: DeepSeek (the whole organization), xAI (the Development department)." */
export function neverUsedWords(list: readonly NeverUsed[]): string | null {
  if (list.length === 0) return null;
  return `Never used here: ${list.map((n) => `${n.company} (${n.from})`).join(", ")}.`;
}
