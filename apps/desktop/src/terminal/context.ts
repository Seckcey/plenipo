import { createContext } from "react";
import type { Environment } from "@plenipo/types";

import type { PanelState, TerminalTab } from "./panel";

export interface TerminalApi {
  panel: PanelState;
  /** The panel's size, kept within the room the page leaves. */
  size: number;
  maxSize: number;
  setSize: (size: number) => void;
  setSide: (side: PanelState["side"]) => void;
  toggle: () => void;
  show: () => void;
  hide: () => void;
  tabs: TerminalTab[];
  active: string | null;
  setActive: (id: string) => void;
  /** A new terminal on this PC, or on a server from Settings → Servers. */
  openHere: () => void;
  openServer: (serverId: string, name: string, environment: Environment) => void;
  close: (id: string) => void;
  /** Watch tabs that opened while the owner looked elsewhere. */
  unseen: number;
  /** Measure the work area (the page and the panel) through this ref. */
  measure: (el: HTMLElement | null) => void;
}

export const TerminalContext = createContext<TerminalApi | null>(null);
