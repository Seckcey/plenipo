import { useEffect, useState } from "react";
import type { ChainOrder } from "@plenipo/types";

import { getChainOrders } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";

/** Ledger events after which a position's chain of command may look different (ADR-202). */
function changesTheChain(type: string): boolean {
  return type.startsWith("chain.") || type === "task.state_changed";
}

/**
 * A position's chain of command, kept current (ADR-202): the owner's orders it was given, that
 * went through it, or that went past it, newest first. `null` while loading, and with no
 * position (a worker's conversation has none).
 */
export function useChainOrders(positionId: string | null): ChainOrder[] | null {
  const [orders, setOrders] = useState<{ id: string; list: ChainOrder[] } | null>(null);
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    subscribeLedgerEvents((event) => {
      if (disposed || !changesTheChain(event.eventType)) return;
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => setRevision((r) => r + 1), 200);
    })
      .then((s) => {
        if (disposed) s();
        else stop = s;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      stop?.();
      if (timer) clearTimeout(timer);
    };
  }, []);
  useEffect(() => {
    if (!positionId) return;
    let cancelled = false;
    getChainOrders(positionId)
      .then((list) => {
        if (!cancelled) setOrders({ id: positionId, list });
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [positionId, revision]);
  return orders && orders.id === positionId ? orders.list : null;
}
