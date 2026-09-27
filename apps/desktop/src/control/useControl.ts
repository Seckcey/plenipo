import { useCallback, useEffect, useRef, useState } from "react";
import type { ControlSession, ControlStatus } from "@plenipo/types";

import {
  allowControl,
  getControlStatus,
  stopAllControl,
  takeOverControl,
  toCommandError,
} from "../api/commands";
import { subscribeControl } from "../api/events";

/**
 * Who uses Plenipo's browser or the mouse and keyboard, kept live (Phase 10), and the owner's
 * controls: stop all, take over, allow again. An update older than the one shown is ignored
 * (updates can arrive out of order). A session the owner took over stays on screen, saying the
 * owner has control, until the owner dismisses it — even after its worker's step has ended.
 */
export function useControl() {
  const [live, setLive] = useState<ControlStatus | null>(null);
  const [kept, setKept] = useState<ControlSession[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const newest = useRef(-1);

  const apply = useCallback((next: ControlStatus) => {
    if (next.revision < newest.current) return;
    newest.current = next.revision;
    setLive(next);
    const taken = next.sessions.filter((s) => s.state === "takenOver");
    if (taken.length > 0) {
      setKept((k) => [...k.filter((x) => !taken.some((t) => t.id === x.id)), ...taken]);
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | null = null;
    void subscribeControl((next) => {
      if (!disposed) apply(next);
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
            if (!disposed) apply(s);
          })
          .catch(() => undefined);
      });
    return () => {
      disposed = true;
      stop?.();
    };
  }, [apply]);

  const act = useCallback(
    async (work: () => Promise<ControlStatus>) => {
      setPending(true);
      setError(null);
      try {
        apply(await work());
      } catch (reason) {
        setError(toCommandError(reason).message);
      } finally {
        setPending(false);
      }
    },
    [apply],
  );

  // The live sessions, plus the ones the owner took over whose step has since ended.
  const status: ControlStatus | null = live && {
    ...live,
    sessions: [...live.sessions, ...kept.filter((k) => !live.sessions.some((s) => s.id === k.id))],
  };

  return {
    status,
    error,
    pending,
    stopAll: () => act(stopAllControl),
    takeOver: (sessionId: string) => act(() => takeOverControl(sessionId)),
    allow: () => act(allowControl),
    dismiss: (sessionId: string) => setKept((k) => k.filter((s) => s.id !== sessionId)),
  };
}

export type Control = ReturnType<typeof useControl>;
