import { useState } from "react";
import type { OrgSnapshot } from "@plenipo/types";

import type { InspectorActions } from "./types";

/** Run a change from the panel: `pending` while it runs, then the refusal to show, if any. */
export function useRun(actions: InspectorActions) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const go = async (work: () => Promise<OrgSnapshot>) => {
    setPending(true);
    setError(null);
    const failure = await actions.run(work);
    setPending(false);
    setError(failure);
    return failure === null;
  };
  /** A change whose answer is not the organization (a rule, learning): the panel reloads. */
  const change = async (work: () => Promise<unknown>) => {
    setPending(true);
    setError(null);
    const failure = await actions.change(work);
    setPending(false);
    setError(failure);
    return failure === null;
  };
  return { pending, error, go, change, setError };
}

export type Run = ReturnType<typeof useRun>;
