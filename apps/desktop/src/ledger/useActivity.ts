import { useCallback, useEffect, useRef, useState } from "react";
import type { ActivityScope, ActivitySeries } from "@plenipo/types";

import { getActivity, toCommandError } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";

const DAY_MS = 86_400_000;
/** 24 hours in 15-minute buckets (ADR-029 §7). */
const BUCKETS = 96;
/** New events redraw the strips at most this often. */
const DEBOUNCE_MS = 1_000;
/** The strips slide forward even when nothing happens. */
const TICK_MS = 60_000;

export interface Activity {
  status: "loading" | "ready" | "error";
  /** One series per scope, in the order asked. */
  series: ActivitySeries[];
  error: string | null;
}

/**
 * The last 24 hours of activity for each scope, from the Ledger, kept live: new events redraw
 * the strips (debounced), and they move forward every minute.
 */
export function useActivity(scopes: readonly ActivityScope[]): Activity {
  const key = JSON.stringify(scopes);
  const [activity, setActivity] = useState<Activity>({
    status: "loading",
    series: [],
    error: null,
  });
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const load = useCallback(async (asked: string) => {
    const wanted = JSON.parse(asked) as ActivityScope[];
    if (wanted.length === 0) return;
    const to = Date.now();
    try {
      const series = await getActivity(wanted, to - DAY_MS, to, BUCKETS);
      setActivity({ status: "ready", series, error: null });
    } catch (reason) {
      setActivity((prev) => ({
        status: prev.status === "ready" ? "ready" : "error",
        series: prev.series,
        error: toCommandError(reason).message,
      }));
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    void load(key);
    subscribeLedgerEvents(() => {
      if (disposed || timer.current) return;
      timer.current = setTimeout(() => {
        timer.current = null;
        if (!disposed) void load(key);
      }, DEBOUNCE_MS);
    })
      .then((stop) => {
        if (disposed) stop();
        else unsubscribe = stop;
      })
      .catch(() => undefined);
    const tick = setInterval(() => void load(key), TICK_MS);
    return () => {
      disposed = true;
      unsubscribe?.();
      clearInterval(tick);
      if (timer.current) clearTimeout(timer.current);
      timer.current = null;
    };
  }, [key, load]);

  return scopes.length === 0 ? { status: "ready", series: [], error: null } : activity;
}
