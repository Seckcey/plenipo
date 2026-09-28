import { useEffect, useState } from "react";
import type { LedgerEvent, LiveView } from "@plenipo/types";

import { getLiveView } from "../api/commands";
import { subscribeControl, subscribeLedgerEvents } from "../api/events";

/** Ledger events after which who works where, or what it touches, may have changed. */
export function affectsLiveView(e: LedgerEvent): boolean {
  const t = e.eventType;
  return (
    t.startsWith("liaison.") ||
    t.startsWith("guard.") ||
    t.startsWith("ssh.") ||
    t.startsWith("control.") ||
    t === "capability.used" ||
    t === "task.state_changed"
  );
}

/** How often the view is read again while someone works (a touch counts for two minutes). */
const REFRESH_MS = 15_000;
const DEBOUNCE_MS = 400;

/**
 * The canvas's live view (ADR-053 §17–§20): read once, then again (debounced) whenever Guard,
 * Liaison, a server, or a task records something, when the control status changes, and every
 * few seconds while anyone is working. `null` until read, or while `on` is false.
 */
export function useLiveView(on: boolean): LiveView | null {
  const [view, setView] = useState<LiveView | null>(null);
  useEffect(() => {
    if (!on) return;
    let live = true;
    let timer: ReturnType<typeof setTimeout> | null = null;
    let busy = false;
    const stops: (() => void)[] = [];
    const read = () => {
      getLiveView()
        .then((v) => {
          busy = v.workers.length > 0 || v.handoffs.length > 0;
          if (live) setView(v);
        })
        .catch(() => undefined);
    };
    const soon = () => {
      if (!live || timer) return;
      timer = setTimeout(() => {
        timer = null;
        read();
      }, DEBOUNCE_MS);
    };
    const keep = (stop: () => void) => {
      if (live) stops.push(stop);
      else stop();
    };
    // Listen first, then read, so nothing between the two is missed.
    void Promise.allSettled([
      subscribeLedgerEvents((e) => {
        if (affectsLiveView(e)) soon();
      }).then(keep),
      subscribeControl(() => soon()).then(keep),
    ]).finally(() => {
      if (live) read();
    });
    const tick = setInterval(() => {
      if (live && busy) read();
    }, REFRESH_MS);
    return () => {
      live = false;
      clearInterval(tick);
      if (timer) clearTimeout(timer);
      stops.forEach((s) => s());
    };
  }, [on]);
  return on ? view : null;
}

/** Whether the system asks for less motion (Windows' animation effects off). */
export function useReducedMotion(): boolean {
  const query = "(prefers-reduced-motion: reduce)";
  const [reduced, setReduced] = useState(
    () => typeof window.matchMedia === "function" && window.matchMedia(query).matches,
  );
  useEffect(() => {
    if (typeof window.matchMedia !== "function") return;
    const m = window.matchMedia(query);
    const on = () => setReduced(m.matches);
    m.addEventListener?.("change", on);
    return () => m.removeEventListener?.("change", on);
  }, []);
  return reduced;
}
