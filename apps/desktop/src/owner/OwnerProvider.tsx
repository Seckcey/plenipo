import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { OwnerProfile, OwnerProfileInput } from "@plenipo/types";

import { getOwnerProfile, setOwnerProfile, toCommandError } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";
import { OwnerContext, type OwnerApi } from "./context";

/** The Ledger event recorded each time your tile changes. */
export const OWNER_CHANGED = "owner.profile_changed";

/**
 * Your tile (Phase 18, ADR-056), shared by the top bar's button and the canvas: loaded when
 * Plenipo starts, and again each time it changes (the Ledger's `owner.profile_changed`), so both
 * always show the same picture, status, mood, and message. A read that failed is tried again with
 * `reload` (the button's panel does, when it opens with nothing loaded).
 */
export function OwnerProvider({ children }: { children: ReactNode }) {
  const [profile, setProfile] = useState<OwnerProfile | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  // Only the newest read counts: an older one that answers late is dropped.
  const ticket = useRef(0);
  const alive = useRef(true);

  const load = useCallback(() => {
    const mine = ++ticket.current;
    setLoadError(null);
    Promise.resolve()
      .then(() => getOwnerProfile())
      .then((next) => {
        if (alive.current && mine === ticket.current && next) setProfile(next);
      })
      .catch((reason: unknown) => {
        if (alive.current && mine === ticket.current) {
          setLoadError(toCommandError(reason).message);
        }
      });
  }, []);

  useEffect(() => {
    alive.current = true;
    let stop: (() => void) | undefined;
    // Listen first, then read: a change made in between is never missed.
    Promise.resolve()
      .then(() =>
        subscribeLedgerEvents((e) => {
          if (alive.current && e.eventType === OWNER_CHANGED) load();
        }),
      )
      .then((unsubscribe) => {
        if (!alive.current) unsubscribe?.();
        else stop = unsubscribe;
      })
      .catch(() => undefined)
      .finally(() => {
        if (alive.current) load();
      });
    return () => {
      alive.current = false;
      stop?.();
    };
  }, [load]);

  const save = useCallback(async (input: OwnerProfileInput) => {
    const next = await setOwnerProfile(input);
    ticket.current++;
    if (alive.current) setProfile(next);
    return next;
  }, []);

  const api = useMemo<OwnerApi>(
    () => ({ profile, save, reload: load, loadError }),
    [profile, save, load, loadError],
  );
  return <OwnerContext.Provider value={api}>{children}</OwnerContext.Provider>;
}
