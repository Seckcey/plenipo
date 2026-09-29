import { useCallback, useEffect, useRef } from "react";
import type { ConnectionsPage } from "@plenipo/types";

import { getConnections } from "../../api/commands";
import { useLive } from "../../guard/usePermissions";

/** Ledger events after which Settings → Connections may look different (Phase 20). */
export function affectsConnections(eventType: string): boolean {
  return (
    eventType.startsWith("connection.") ||
    eventType === "guard.connection_refused" ||
    eventType === "guard.switches_changed" ||
    eventType.startsWith("org.role_") ||
    eventType.startsWith("org.position_")
  );
}

/** How long the page waits between looks while a sign-in waits in your browser. */
export const SIGN_IN_POLL_MS = 1_500;

/**
 * Settings → Connections, kept live; looked at again every moment while a sign-in waits — one
 * look at a time, and a look that started before your last change never puts back what was on
 * screen before it.
 */
export function useConnections() {
  const { value, error, reload, apply } = useLive<ConnectionsPage>(
    getConnections,
    affectsConnections,
  );
  const changes = useRef(0);
  const applyChange = useCallback(
    (page: ConnectionsPage) => {
      changes.current += 1;
      apply(page);
    },
    [apply],
  );
  const waiting = value?.services.some((s) => s.connections.some((c) => c.signingIn)) ?? false;
  useEffect(() => {
    if (!waiting) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const look = () => {
      timer = setTimeout(() => {
        const seen = changes.current;
        getConnections()
          .then((page) => {
            if (!stopped && seen === changes.current) apply(page);
          })
          .catch(() => undefined)
          .finally(() => {
            if (!stopped) look();
          });
      }, SIGN_IN_POLL_MS);
    };
    look();
    return () => {
      stopped = true;
      if (timer) clearTimeout(timer);
    };
  }, [waiting, apply]);
  return { page: value, error, reload, apply: applyChange };
}
