import { useCallback, useEffect, useRef, useState } from "react";
import type { CommunityView } from "@plenipo/types";

import { getCommunity, toCommandError } from "../api/commands";
import { subscribeCommunity } from "../api/events";

/**
 * Community, kept live (Phase 24): read when shown, and again whenever it changes (the switch, a
 * code shown or used, joining, signing out, leaving, 8 West opening or closing it), in any window.
 */
export function useCommunity(): {
  view: CommunityView | null;
  error: string | null;
  reload: () => void;
  show: (view: CommunityView) => void;
} {
  const [view, setView] = useState<CommunityView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);
  const reload = useCallback(() => {
    getCommunity()
      .then((v) => {
        if (!alive.current) return;
        setView(v);
        setError(null);
      })
      .catch((reason: unknown) => {
        if (alive.current) setError(toCommandError(reason).message);
      });
  }, []);
  useEffect(() => {
    alive.current = true;
    let stop: (() => void) | null = null;
    void subscribeCommunity(reload)
      .then((s) => {
        if (alive.current) stop = s;
        else s();
      })
      .catch(() => undefined)
      .finally(reload);
    return () => {
      alive.current = false;
      stop?.();
    };
  }, [reload]);
  return { view, error, reload, show: setView };
}
