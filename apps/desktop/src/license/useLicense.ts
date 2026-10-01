import { useCallback, useEffect, useRef, useState } from "react";
import type { LicenseView } from "@plenipo/types";

import { getLicense, toCommandError } from "../api/commands";
import { subscribeLicense } from "../api/events";

/**
 * The PC's license, kept live (Phase 11A): read when shown, again whenever it changes (a key
 * entered or removed in any window, a check, Pro starting or ending), and when the window comes
 * to the front (Pro can end by the clock).
 */
export function useLicense(): {
  view: LicenseView | null;
  error: string | null;
  reload: () => void;
  show: (view: LicenseView) => void;
} {
  const [view, setView] = useState<LicenseView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);
  const reload = useCallback(() => {
    getLicense()
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
    void subscribeLicense(reload)
      .then((s) => {
        if (alive.current) stop = s;
        else s();
      })
      .catch(() => undefined)
      .finally(reload);
    window.addEventListener("focus", reload);
    return () => {
      alive.current = false;
      stop?.();
      window.removeEventListener("focus", reload);
    };
  }, [reload]);
  return { view, error, reload, show: setView };
}

/** Whether this PC is on Free (false while loading: a note appears only once it is known). */
export function useOnFree(): boolean {
  return useLicense().view?.edition === "free";
}
