import type { OrgSnapshot } from "@plenipo/types";

import { runtimeChoiceLabel } from "./runtimeChoices";

/**
 * The AI tools a picker offers (Phase 25, item 1.5): your subscriptions first, then the AI tools
 * paid per use with your key, once a key works. `current` stays listed even when it no longer
 * would be, so a saved choice still shows.
 */
export function RuntimeOptions({ snapshot, current }: { snapshot: OrgSnapshot; current: string }) {
  const subscriptions = snapshot.runtimes.filter((r) => !r.paid);
  const paid = snapshot.runtimes.filter((r) => r.paid && (r.ready || r.id === current));
  return (
    <>
      <optgroup label="Your subscriptions">
        {subscriptions.map((r) => (
          <option key={r.id} value={r.id}>
            {runtimeChoiceLabel(snapshot, r.id)}
          </option>
        ))}
      </optgroup>
      {paid.length > 0 && (
        <optgroup label="Paid per use with your key">
          {paid.map((r) => (
            <option key={r.id} value={r.id}>
              {runtimeChoiceLabel(snapshot, r.id)}
            </option>
          ))}
        </optgroup>
      )}
    </>
  );
}
