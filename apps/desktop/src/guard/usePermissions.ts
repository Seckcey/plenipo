import { useCallback, useEffect, useRef, useState } from "react";
import type { ApprovalQueue, PermissionSet, PermissionsSnapshot } from "@plenipo/types";

import { getApprovals, getPermissions, toCommandError } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";

/** Ledger events after which the Permissions page may look different. */
export function affectsPermissions(eventType: string): boolean {
  return (
    eventType.startsWith("guard.") ||
    eventType.startsWith("vault.") ||
    eventType.startsWith("approval.") ||
    eventType === "capability.used" ||
    eventType === "org.role_created" ||
    eventType === "org.role_renamed" ||
    eventType.startsWith("org.department_") ||
    eventType.startsWith("org.project_")
  );
}

/** Ledger events after which the approval queue may look different. */
export function affectsApprovals(eventType: string): boolean {
  return eventType.startsWith("approval.") || eventType.startsWith("guard.grant_");
}

/** A value from Core kept live: reloaded (debounced) after relevant Ledger events; a change
 * applies what its command returns. */
function useLive<T>(load: () => Promise<T>, relevant: (eventType: string) => boolean) {
  const [value, setValue] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const reload = useCallback(async () => {
    try {
      setValue(await load());
      setError(null);
    } catch (reason) {
      setError(toCommandError(reason).message);
    }
  }, [load]);

  const apply = useCallback((next: T) => {
    setValue(next);
    setError(null);
  }, []);

  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | null = null;
    const schedule = () => {
      if (disposed) return;
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => void reload(), 120);
    };
    void subscribeLedgerEvents((event) => {
      if (relevant(event.eventType)) schedule();
    })
      .then((s) => {
        if (disposed) s();
        else stop = s;
      })
      .catch(() => undefined)
      .finally(() => {
        if (!disposed) void reload();
      });
    return () => {
      disposed = true;
      stop?.();
      if (timer.current) clearTimeout(timer.current);
    };
  }, [reload, relevant]);

  return { value, error, reload, apply };
}

/** Settings → Permissions, kept live. */
export function usePermissions() {
  const { value, error, reload, apply } = useLive<PermissionsSnapshot>(
    getPermissions,
    affectsPermissions,
  );
  return { snapshot: value, error, reload, apply };
}

/** The approval queue, kept live. */
export function useApprovals() {
  const { value, error, reload, apply } = useLive<ApprovalQueue>(getApprovals, affectsApprovals);
  return { queue: value, error, reload, apply };
}

/** The permission sets, loaded once (for a form that offers them). `null` until loaded, or if
 * they cannot be read. */
export function usePermissionSets(): PermissionSet[] | null {
  const [sets, setSets] = useState<PermissionSet[] | null>(null);
  useEffect(() => {
    let live = true;
    Promise.resolve()
      .then(getPermissions)
      .then(
        (s) => {
          if (live) setSets(s.settings.sets);
        },
        () => undefined,
      );
    return () => {
      live = false;
    };
  }, []);
  return sets;
}
