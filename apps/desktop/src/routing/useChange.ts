import { useState } from "react";
import type { RoutingSnapshot } from "@plenipo/types";

import { toCommandError } from "../api/commands";

export type Apply = (snapshot: RoutingSnapshot) => void;

/** Run a change to the model settings; applies the snapshot it returns, or keeps the refusal
 * to show. Resolves true once applied. */
export function useChange(onApply: Apply) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = async (work: () => Promise<RoutingSnapshot>): Promise<boolean> => {
    setPending(true);
    setError(null);
    try {
      onApply(await work());
      return true;
    } catch (reason) {
      setError(toCommandError(reason).message);
      return false;
    } finally {
      setPending(false);
    }
  };
  return { pending, error, run };
}
