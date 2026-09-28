import { createContext } from "react";

import type { DropState } from "../../org/nodes";

export interface ZoomControls {
  level: number;
  zoomIn: () => void;
  zoomOut: () => void;
  fit: () => void;
}

/** What the canvas gives its toolbar: the zoom, and whether a drag over the trash can may drop. */
export interface CanvasControls {
  zoom: ZoomControls;
  trash: DropState;
}

export const CanvasControlsContext = createContext<CanvasControls | null>(null);
