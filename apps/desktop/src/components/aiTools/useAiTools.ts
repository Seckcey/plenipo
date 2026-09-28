import { useCallback, useEffect, useRef, useState } from "react";
import type { AiToolsPage, AiToolUsage, LedgerEvent } from "@plenipo/types";

import { getAiTools, getAiToolUsage, toCommandError } from "../../api/commands";
import { subscribeAgentUpdates, subscribeLedgerEvents } from "../../api/events";
import { MOVING, usageWindow, type UsageWindow } from "./words";

/** How often the page reads itself again while an update or a check is going on. */
const MOVING_MS = 2000;

/** Ledger events after which the AI tools page may look different. */
export function changesAiTools(e: LedgerEvent): boolean {
  return (
    e.eventType.startsWith("ai_tool.") ||
    e.eventType.startsWith("ai_tools.") ||
    e.eventType === "terminal.closed"
  );
}

/**
 * The AI tools page's own part (Phase 19): read when the page opens, again (debounced) after the
 * Ledger's AI tool events, each task's result, and each AI tool check or plan report, and every 2
 * seconds while an update or a check is going on. A change made on the page applies the page its command returns.
 * `usageRevision` moves on after each task's result, so the usage is read again.
 */
export function useAiTools() {
  const [page, setPage] = useState<AiToolsPage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [usageRevision, setUsageRevision] = useState(0);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const reload = useCallback(async () => {
    try {
      setPage(await getAiTools());
      setError(null);
    } catch (reason) {
      setError(toCommandError(reason).message);
    }
  }, []);

  const apply = useCallback((next: AiToolsPage) => {
    setPage(next);
    setError(null);
  }, []);

  useEffect(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    const schedule = () => {
      if (disposed) return;
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => void reload(), 150);
    };
    const keep = (stop: () => void) => {
      if (disposed) stop();
      else stops.push(stop);
    };
    void Promise.allSettled([
      subscribeLedgerEvents((event) => {
        if (disposed) return;
        if (changesAiTools(event)) schedule();
        // A task finished: its tokens count, and a tool may have reported its plan.
        if (event.eventType === "agent.result") {
          schedule();
          setUsageRevision((n) => n + 1);
        }
      }).then(keep),
      subscribeAgentUpdates((update) => {
        if (update.kind === "runtimes" || update.kind === "plan") schedule();
      }).then(keep),
    ]).finally(() => {
      if (!disposed) void reload();
    });
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
      if (timer.current) clearTimeout(timer.current);
    };
  }, [reload]);

  const moving =
    page !== null &&
    (page.looking || page.tools.some((t) => t.checking || MOVING.has(t.update.state)));
  useEffect(() => {
    if (!moving) return;
    const every = setInterval(() => void reload(), MOVING_MS);
    return () => clearInterval(every);
  }, [moving, reload]);

  return { page, error, reload, apply, usageRevision };
}

/**
 * One AI tool's usage from last week's Monday (or 14 days ago) to today, with the days it was
 * read for. Read again when `revision` moves on (a task finished).
 */
export function useToolUsage(runtimeId: string, revision: number) {
  const [state, setState] = useState<{
    usage: AiToolUsage | null;
    window: UsageWindow | null;
    error: string | null;
  }>({ usage: null, window: null, error: null });
  useEffect(() => {
    let live = true;
    const window = usageWindow(Date.now());
    Promise.resolve()
      .then(() => getAiToolUsage(runtimeId, window.starts))
      .then(
        (usage) => {
          if (live) setState({ usage, window, error: null });
        },
        (reason: unknown) => {
          if (live) setState((s) => ({ ...s, error: toCommandError(reason).message }));
        },
      );
    return () => {
      live = false;
    };
  }, [runtimeId, revision]);
  return state;
}
