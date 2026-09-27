import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { IndicatorView } from "./views/IndicatorView";
import "./styles.css";

const container = document.getElementById("root");
if (!container) throw new Error("Root element #root not found");

// The sign window shown while a worker uses the mouse and keyboard (Phase 10) loads this same
// page as `index.html#indicator`.
const indicator = window.location.hash === "#indicator";

createRoot(container).render(<StrictMode>{indicator ? <IndicatorView /> : <App />}</StrictMode>);
