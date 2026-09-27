import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyTheme, readTheme } from "@plenipo/ui";

import { App } from "./App";
import { IndicatorView } from "./views/IndicatorView";
// The design system first (tokens and components, ADR-029), then the app's page layouts.
import "@plenipo/ui/styles.css";
import "./styles.css";

const container = document.getElementById("root");
if (!container) throw new Error("Root element #root not found");

// The remembered theme, before the first paint (dark unless you chose light).
applyTheme(readTheme());

// The sign window shown while a worker uses the mouse and keyboard (Phase 10) loads this same
// page as `index.html#indicator`.
const indicator = window.location.hash === "#indicator";

createRoot(container).render(<StrictMode>{indicator ? <IndicatorView /> : <App />}</StrictMode>);
