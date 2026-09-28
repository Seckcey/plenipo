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
    eventType === "org.role_created" ||
    eventType === "org.role_renamed" ||
    eventType.startsWith("org.position_")
  );
}

/** How often the page looks again while a sign-in waits in your browser. */
export const SIGN_IN_POLL_MS = 1_500;

/** Settings → Connections, kept live; looked at again every moment while a sign-in waits. */
export function useConnections() {
  const { value, error, reload, apply } = useLive<ConnectionsPage>(
    getConnections,
    affectsConnections,
  );
  const waiting = value?.services.some((s) => s.connections.some((c) => c.signingIn)) ?? false;
  useEffect(() => {
    if (!waiting) return;
    const timer = setInterval(() => void reload(), SIGN_IN_POLL_MS);
    return () => clearInterval(timer);
  }, [waiting, reload]);
  return { page: value, error, reload, apply };
}
