import { useCallback, useEffect, useRef, useState } from "react";
import type { ActivityScope, ActivitySeries } from "@plenipo/types";

import { getActivity, toCommandError } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";

const DAY_MS = 86_400_000;
/** 24 hours in 15-minute buckets (ADR-030 §7). */
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
  // Each answer is kept with the scopes it was for, so a late answer for an older list of
  // scopes is never shown against a newer one.
  const [activity, setActivity] = useState<Activity & { key: string }>({
    key: "",
    status: "loading",
    series: [],
    error: null,
  });
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  /** One request at a time: a reload asked for meanwhile runs once, when it ends. */
  const inFlight = useRef(false);
  const again = useRef<string | null>(null);
  /** The scopes asked for most recently. */
  const latest = useRef(key);

  const load = useCallback(async (asked: string) => {
    const wanted = JSON.parse(asked) as ActivityScope[];
    if (wanted.length === 0) return;
    if (inFlight.current) {
      again.current = asked;
      return;
    }
    inFlight.current = true;
    const to = Date.now();
    try {
      const series = await getActivity(wanted, to - DAY_MS, to, BUCKETS);
      if (asked === latest.current)
        setActivity({ key: asked, status: "ready", series, error: null });
    } catch (reason) {
      if (asked === latest.current) {
        setActivity((prev) => ({
          key: asked,
          status: prev.key === asked && prev.status === "ready" ? "ready" : "error",
          series: prev.key === asked ? prev.series : [],
          error: toCommandError(reason).message,
        }));
      }
    } finally {
      inFlight.current = false;
      const next = again.current;
      again.current = null;
      if (next !== null) void load(next);
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    latest.current = key;
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

  if (scopes.length === 0) return { status: "ready", series: [], error: null };
  // Until the answer for these scopes arrives, they are loading (never another list's strips).
  if (activity.key !== key) return { status: "loading", series: [], error: null };
  return { status: activity.status, series: activity.series, error: activity.error };
}
