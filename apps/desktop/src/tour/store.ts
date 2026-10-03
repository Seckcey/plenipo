/**
 * What the setup tour remembers (Phase 25, item 2.9; ADR-198): where you stopped in each
 * organization, on this computer, and whether it is showing in this window now (the canvas's
 * own tour waits while it does).
 */
import { useSyncExternalStore } from "react";

export type TourStatus = "going" | "stopped" | "finished";

export interface TourProgress {
  /** The step you were on. */
  step: number;
  status: TourStatus;
}

export const SETUP_TOUR_KEY = "plenipo.setupTour";
/** "off": the tour never starts by itself on this PC (the end-to-end tests turn it off). */
export const BY_ITSELF_KEY = "plenipo.setupTourByItself";

export function startsByItselfHere(): boolean {
  try {
    return localStorage.getItem(BY_ITSELF_KEY) !== "off";
  } catch {
    return true;
  }
}

const isProgress = (v: unknown): v is TourProgress =>
  typeof v === "object" &&
  v !== null &&
  typeof (v as TourProgress).step === "number" &&
  ["going", "stopped", "finished"].includes((v as TourProgress).status);

function readAll(): Record<string, unknown> {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(SETUP_TOUR_KEY) ?? "{}");
    return typeof parsed === "object" && parsed !== null ? (parsed as Record<string, unknown>) : {};
  } catch {
    return {};
  }
}

/** Where the tour stopped in this organization; `null`: never taken here. */
export function progressOf(org: string): TourProgress | null {
  const p = readAll()[org];
  return isProgress(p) ? p : null;
}

export function saveProgress(org: string, progress: TourProgress) {
  try {
    localStorage.setItem(SETUP_TOUR_KEY, JSON.stringify({ ...readAll(), [org]: progress }));
  } catch {
    // Storage unavailable: it is simply not remembered.
  }
  emit();
}

// This window: the organization it shows, and whether the tour is showing.
let state: { org: string | null; running: boolean } = { org: null, running: false };
const listeners = new Set<() => void>();
const emit = () => {
  state = { ...state };
  listeners.forEach((l) => l());
};
const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

/** The organization this window shows (set by the tour once the list is loaded). */
export function setTourOrganization(org: string | null) {
  if (state.org === org) return;
  state.org = org;
  emit();
}

/** Start the tour, or pick it up where you stopped. */
export function startSetupTour() {
  if (state.running) return;
  state.running = true;
  emit();
}

export function endSetupTour() {
  if (!state.running) return;
  state.running = false;
  emit();
}

/** Whether the setup tour is showing in this window. */
export function useSetupTourRunning(): boolean {
  return useSyncExternalStore(subscribe, () => state.running);
}

/**
 * The words on the button that starts the tour: "Pick up the setup tour" after you stopped it
 * part way, "Take the setup tour again" once taken, else "Take the setup tour".
 */
export function useSetupTourLabel(): string {
  const org = useSyncExternalStore(subscribe, () => state.org);
  // Read on every change of the store (a save emits), so the words stay current.
  useSyncExternalStore(subscribe, () => state);
  const progress = org ? progressOf(org) : null;
  if (progress?.status === "stopped" && progress.step > 0) return "Pick up the setup tour";
  return progress ? "Take the setup tour again" : "Take the setup tour";
}
