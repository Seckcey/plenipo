import { useCallback, useEffect, useState } from "react";
import type { ControlStatus } from "@plenipo/types";

import {
  allowControl,
  getControlStatus,
  stopAllControl,
  takeOverControl,
  toCommandError,
} from "../api/commands";
import { subscribeControl } from "../api/events";

/** Who uses Plenipo's browser or the mouse and keyboard, kept live (Phase 10), and the owner's
 * controls: stop all, take over, allow again. */
export function useControl() {
  const [status, setStatus] = useState<ControlStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | null = null;
    void subscribeControl((next) => {
      if (!disposed) setStatus(next);
    })
      .then((s) => {
        if (disposed) s();
        else stop = s;
      })
      .catch(() => undefined)
      .finally(() => {
        if (disposed) return;
        getControlStatus()
          .then((s) => {
            if (!disposed) setStatus(s);
          })
          .catch(() => undefined);
      });
    return () => {
      disposed = true;
      stop?.();
    };
  }, []);

  const act = useCallback(async (work: () => Promise<ControlStatus>) => {
    setPending(true);
    setError(null);
    try {
      setStatus(await work());
    } catch (reason) {
      setError(toCommandError(reason).message);
    } finally {
      setPending(false);
    }
  }, []);

  return {
    status,
    error,
    pending,
    stopAll: () => act(stopAllControl),
    takeOver: (sessionId: string) => act(() => takeOverControl(sessionId)),
    allow: () => act(allowControl),
  };
}

export type Control = ReturnType<typeof useControl>;
