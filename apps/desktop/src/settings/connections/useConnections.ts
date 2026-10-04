import { useEffect } from "react";
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
 * look at a time. A look or reload that started before your last change, or before a newer one,
 * never puts back what was on screen before it.
 */
export function useConnections() {
  const { value, error, reload, apply, quietLook } = useLive<ConnectionsPage>(
    getConnections,
    affectsConnections,
  );
  const waiting = value?.services.some((s) => s.connections.some((c) => c.signingIn)) ?? false;
  useEffect(() => {
    if (!waiting) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const next = () => {
      timer = setTimeout(() => {
        void quietLook().finally(() => {
          if (!stopped) next();
        });
      }, SIGN_IN_POLL_MS);
    };
    next();
    return () => {
      stopped = true;
      if (timer) clearTimeout(timer);
    };
  }, [waiting, quietLook]);
  return { page: value, error, reload, apply };
}
