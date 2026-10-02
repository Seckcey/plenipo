import { useCallback, useEffect, useRef, useState } from "react";
import type { RemoteSettings } from "@plenipo/types";

import { getRemote, toCommandError } from "../api/commands";
import { subscribeRemote } from "../api/events";

/**
 * Phone access, kept live (Phase 14): read when shown, and again whenever it changes (a phone
 * added, signed in, or removed; the switch; adding a phone; the relay's connection), in any window.
 */
export function useRemote(): {
  settings: RemoteSettings | null;
  error: string | null;
  reload: () => void;
  show: (settings: RemoteSettings) => void;
} {
  const [settings, setSettings] = useState<RemoteSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);
  const reload = useCallback(() => {
    getRemote()
      .then((s) => {
        if (!alive.current) return;
        setSettings(s);
        setError(null);
      })
      .catch((reason: unknown) => {
        if (alive.current) setError(toCommandError(reason).message);
      });
  }, []);
  useEffect(() => {
    alive.current = true;
    let stop: (() => void) | null = null;
    void subscribeRemote(reload)
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
  return { settings, error, reload, show: setSettings };
}
