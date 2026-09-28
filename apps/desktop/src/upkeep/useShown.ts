import { useCallback, useState } from "react";

import type { Live } from "../pages/useLive";

/**
 * What a section shows: the answer to the owner's last action, until Core's next refresh
 * arrives; then Core's live value again, so a later change is never hidden behind an old answer.
 */
export function useShown<T>(live: Live<T>): [T | null, (value: T) => void] {
  // The answer, and the live value it was given over: a newer live value wins.
  const [shown, setShown] = useState<{ value: T; over: T | null } | null>(null);
  const current = shown !== null && shown.over === live.value ? shown.value : live.value;
  const show = useCallback((value: T) => setShown({ value, over: live.value }), [live.value]);
  return [current, show];
}
