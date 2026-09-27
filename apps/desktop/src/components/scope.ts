import type { OrgSnapshot } from "@plenipo/types";
import type { ScopeOption } from "@plenipo/ui";

/** The whole organization, in the "Showing" picker. */
export const ALL_SCOPE = "all";

/** "department:ID" or "project:ID". */
export type ScopeId = string;

export function scopeOptions(org: OrgSnapshot | null): ScopeOption[] {
  const all: ScopeOption = {
    id: ALL_SCOPE,
    label: org?.name ? `All of ${org.name}` : "Everything",
    kind: "organization",
  };
  if (!org) return [all];
  return [
    all,
    ...org.departments
      .filter((d) => d.active)
      .map((d): ScopeOption => ({ id: `department:${d.id}`, label: d.name, kind: "department" })),
    ...org.projects
      .filter((p) => p.active)
      .map((p): ScopeOption => ({ id: `project:${p.id}`, label: p.name, kind: "project" })),
  ];
}
