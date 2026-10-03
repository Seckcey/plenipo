import { useCallback, useEffect, useRef, useState } from "react";
import type { GettingStarted } from "@plenipo/types";

import {
  closeCommunityGettingStarted,
  communityGettingStarted,
  toCommandError,
} from "../api/commands";

/** Whether **Getting started** is still there: not closed, and a step is left. */
export function gettingStartedShows(state: GettingStarted | null): state is GettingStarted {
  return (
    state !== null && !state.closed && !(state.profile && state.foundSomeone && state.sentAMessage)
  );
}

/**
 * **Getting started**, kept up to date (Phase 24, ADR-169 §5, ADR-172 §4). It is read when the People
 * tab shows (`active`), and again whenever your messages change (`changes`: a message was sent or
 * came, which can finish **Send a message**) and when `reload` is called (a **Find someone** was
 * done). It is kept on this PC and nothing is sent, so reading it as often as that costs nothing.
 * When it can't be read there is nothing to show, and nothing is said.
 */
export function useGettingStarted(
  active: boolean,
  changes: number,
): {
  state: GettingStarted | null;
  /** What went wrong when closing it, in plain words. */
  error: string | null;
  reload: () => void;
  /** Close it for good. Resolves `true` when it is closed. */
  close: () => Promise<boolean>;
} {
  const [state, setState] = useState<GettingStarted | null>(null);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);
  // An answer for a question that was since asked again is dropped, so it never goes back.
  const turn = useRef(0);
  const reload = useCallback(() => {
    const mine = ++turn.current;
    communityGettingStarted()
      .then((next) => {
        if (alive.current && mine === turn.current) setState(next);
      })
      .catch(() => undefined);
  }, []);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  useEffect(() => {
    if (active) reload();
  }, [active, changes, reload]);
  const close = useCallback(() => {
    const mine = ++turn.current;
    setError(null);
    return closeCommunityGettingStarted().then(
      (next) => {
        if (alive.current && mine === turn.current) setState(next);
        return true;
      },
      (reason: unknown) => {
        if (alive.current) setError(toCommandError(reason).message);
        return false;
      },
    );
  }, []);
  return { state, error, reload, close };
}
