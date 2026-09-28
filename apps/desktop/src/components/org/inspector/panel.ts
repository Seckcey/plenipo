/** The properties panel's tabs and its width (Phase 17). */
import { useStoredState } from "@plenipo/ui";

/** The panel's width: remembered, between these (pixels). */
export const PANEL_MIN = 320;
export const PANEL_MAX = 720;
export const PANEL_DEFAULT = 360;
const WIDTH_KEY = "plenipo.panelWidth";

export type PanelTab = "overview" | "job" | "model" | "work" | "team" | "manage";

export const PANEL_TABS: { value: PanelTab; label: string }[] = [
  { value: "overview", label: "Overview" },
  { value: "job", label: "Job" },
  { value: "model", label: "AI model" },
  { value: "work", label: "Work" },
  { value: "team", label: "Team" },
  { value: "manage", label: "Manage" },
];

const isWidth = (v: unknown): v is number =>
  typeof v === "number" && v >= PANEL_MIN && v <= PANEL_MAX;

/** The panel's width, remembered between visits. */
export function usePanelWidth(): [number, (next: number) => void] {
  return useStoredState<number>(WIDTH_KEY, PANEL_DEFAULT, isWidth);
}
