import type { OrgSnapshot } from "@plenipo/types";
import { ScopeSelector } from "@plenipo/ui";

import { ALL_SCOPE, scopeOptions, type ScopeId } from "./scope";

/**
 * Where you are: all of the organization, a department, or a project (top bar, Phase 12A).
 * A scope that no longer exists reads as the whole organization.
 */
export function ScopePicker({
  org,
  value,
  onChange,
}: {
  org: OrgSnapshot | null;
  value: ScopeId;
  onChange: (scope: ScopeId) => void;
}) {
  const options = scopeOptions(org);
  const current = options.some((o) => o.id === value) ? value : ALL_SCOPE;
  return <ScopeSelector options={options} value={current} onChange={onChange} />;
}
