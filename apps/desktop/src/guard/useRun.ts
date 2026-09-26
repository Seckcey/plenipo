import { useState } from "react";

import { toCommandError } from "../api/commands";

/** Run a change; apply what it returns, or keep the refusal to show. Resolves true once
 * applied. */
export function useRun<T>(onApply: (value: T) => void) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = async (work: () => Promise<T>): Promise<boolean> => {
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
