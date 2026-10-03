import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyTheme, readTheme, THEME_KEY } from "@plenipo/ui";

import { getAppInfo, getOrganizations } from "./api/commands";
import { App } from "./App";
import { rememberFor } from "./orgs/storage";
import { setSystemWords } from "./system/words";
import { IndicatorView } from "./views/IndicatorView";
// The design system first (tokens and components, ADR-030), then the app's page layouts.
import "@plenipo/ui/styles.css";
import "./styles.css";

const container = document.getElementById("root");
if (!container) throw new Error("Root element #root not found");

// The remembered theme, before the first paint (dark unless you chose light). A change made in
// another window (the main window, while the sign window is open) follows here at once.
applyTheme(readTheme());
window.addEventListener("storage", (e) => {
  if (e.key === THEME_KEY) applyTheme(readTheme());
});

// The sign window shown while a worker uses the mouse and keyboard (Phase 10) loads this same
// page as `index.html#indicator`.
const indicator = window.location.hash === "#indicator";

/**
 * Which organization this window shows, before the first render: each organization remembers its
 * own page, map, and panels (Phase 21, ADR-094). The sign window shows none.
 */
async function learnOrganization(): Promise<void> {
  if (indicator) return;
  try {
    const listing = await getOrganizations();
    const here = listing.organizations.find((o) => o.id === listing.current);
    rememberFor(listing.current, here?.first ?? true);
  } catch {
    // Not answered: the first organization's names, as before.
  }
}

/**
 * Which system this is, in its own words (Phase 23, ADR-155): "this Mac", not "this PC". The
 * Rust side decides, before the first render. The sign window shows none.
 */
async function learnWords(): Promise<void> {
  if (indicator) return;
  try {
    setSystemWords((await getAppInfo()).words);
  } catch {
    // Not answered: Windows' words, as before.
  }
}

void Promise.all([learnOrganization(), learnWords()]).then(() => {
  createRoot(container).render(<StrictMode>{indicator ? <IndicatorView /> : <App />}</StrictMode>);
});
