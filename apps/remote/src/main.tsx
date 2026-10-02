import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyTheme, readTheme } from "@plenipo/ui";

import { App } from "./App";
import { browserKeep } from "./keep";
// The design system first (tokens and components, ADR-030), then the page's own layout.
import "@plenipo/ui/styles.css";
import "./styles.css";

const container = document.getElementById("root");
if (!container) throw new Error("Root element #root not found");

// The remembered theme, before the first paint.
applyTheme(readTheme());

// The background part that shows notices when the page is closed (part 14C): the built page only.
if (import.meta.env.PROD && "serviceWorker" in navigator) {
  void navigator.serviceWorker.register("/sw.js").catch(() => undefined);
}

createRoot(container).render(
  <StrictMode>
    <App keep={browserKeep()} />
  </StrictMode>,
);
