import type { ServersSnapshot } from "@plenipo/types";

import { getServers } from "../api/commands";
import { useLive } from "../guard/usePermissions";

/** Ledger events after which Settings → Servers may look different (Phase 11). */
export function affectsServers(eventType: string): boolean {
  return (
    eventType.startsWith("guard.server_") ||
    eventType === "guard.switches_changed" ||
    eventType.startsWith("ssh.") ||
    eventType === "vault.server_sign_in_stored" ||
    eventType === "guard.role_assigned" ||
    eventType.startsWith("guard.set_") ||
    eventType.startsWith("guard.grant_") ||
    eventType === "org.role_created" ||
    eventType === "org.role_renamed"
  );
}

/** Settings → Servers, kept live. */
export function useServers() {
  const { value, error, reload, apply } = useLive<ServersSnapshot>(getServers, affectsServers);
  return { snapshot: value, error, reload, apply };
}
