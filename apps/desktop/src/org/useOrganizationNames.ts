import { useCallback, useEffect, useRef, useState } from "react";
import type { OrgSnapshot } from "@plenipo/types";

import { getOrganization } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";

/**
 * The organization for the frame (the top bar's "Showing" picker): its departments and
 * projects. It reloads only when the organization itself changes (`org.*`), not on every
 * task, so the frame does not redraw with each worker's step. Pages that show live work use
 * `useOrganization`.
 */
export function useOrganizationNames(): { snapshot: OrgSnapshot | null } {
  const [snapshot, setSnapshot] = useState<OrgSnapshot | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const reload = useCallback(async () => {
    try {
      setSnapshot(await getOrganization());
    } catch {
      // The picker keeps what it had; pages report their own errors.
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    subscribeLedgerEvents((event) => {
      if (disposed || !event.eventType.startsWith("org.")) return;
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => void reload(), 300);
    })
      .then((stop) => {
        if (disposed) stop();
        else unsubscribe = stop;
      })
      .catch(() => undefined)
      .finally(() => {
        if (!disposed) void reload();
      });
    return () => {
      disposed = true;
      unsubscribe?.();
      if (timer.current) clearTimeout(timer.current);
    };
  }, [reload]);

  return { snapshot };
}
