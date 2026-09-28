/* global document, window */

// The full page remains usable without JavaScript. Enhance its anchor links into
// independent, keyboard-accessible tab sets only after finding every panel.
for (const group of document.querySelectorAll("[data-tabs]")) {
  const tablist = group.querySelector("[data-tablist]");
  const tabs = [...tablist.querySelectorAll("[data-tab]")];
  const panels = tabs.map((tab) => document.getElementById(tab.hash.slice(1)));
  if (panels.some((panel) => !panel)) continue;

  const activate = (index, focus = false) => {
    tabs.forEach((tab, tabIndex) => {
      const selected = tabIndex === index;
      tab.setAttribute("aria-selected", String(selected));
      tab.tabIndex = selected ? 0 : -1;
      panels[tabIndex].hidden = !selected;
    });
    if (focus) tabs[index].focus();
  };

  tablist.setAttribute("role", "tablist");
  tabs.forEach((tab, index) => {
    tab.setAttribute("role", "tab");
    tab.setAttribute("aria-controls", panels[index].id);
    panels[index].setAttribute("role", "tabpanel");
    panels[index].setAttribute("aria-labelledby", tab.id);
    panels[index].tabIndex = 0;
    tab.addEventListener("click", (event) => {
      event.preventDefault();
      activate(index);
    });
    tab.addEventListener("keydown", (event) => {
      let next;
      if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
      else if (event.key === "ArrowLeft") next = (index - 1 + tabs.length) % tabs.length;
      else if (event.key === "Home") next = 0;
      else if (event.key === "End") next = tabs.length - 1;
      else if (event.key === " ") next = index;
      else return;
      event.preventDefault();
      activate(next, true);
    });
  });
  const initial = panels.findIndex((panel) => `#${panel.id}` === window.location.hash);
  activate(initial < 0 ? 0 : initial);
  group.dataset.enhanced = "true";
}

const header = document.querySelector(".site-header");
const menuButton = document.querySelector(".menu-toggle");
const navigation = document.getElementById("main-navigation");
const menuLabel = menuButton.querySelector(".sr-only");

const setMenuOpen = (open) => {
  menuButton.setAttribute("aria-expanded", String(open));
  menuLabel.textContent = open ? "Close navigation" : "Open navigation";
  navigation.classList.toggle("is-open", open);
};

header.dataset.enhanced = "true";
menuButton.hidden = false;
menuButton.addEventListener("click", () => {
  setMenuOpen(menuButton.getAttribute("aria-expanded") !== "true");
});
navigation.addEventListener("click", (event) => {
  if (event.target.closest("a")) setMenuOpen(false);
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && menuButton.getAttribute("aria-expanded") === "true") {
    setMenuOpen(false);
    menuButton.focus();
  }
});
document.addEventListener("click", (event) => {
  if (!header.contains(event.target)) setMenuOpen(false);
});
window.matchMedia("(min-width: 821px)").addEventListener("change", () => setMenuOpen(false));

// Opening an FAQ deep link reveals its answer and preserves native details behavior.
const openLinkedQuestion = () => {
  const target = document.getElementById(window.location.hash.slice(1));
  if (target?.matches("details")) target.open = true;
};
window.addEventListener("hashchange", openLinkedQuestion);
openLinkedQuestion();

// Open the sample on page entry. The static example remains readable while it
// loads and when scripts fail; returning to it never starts another automatic load.
const startDemo = document.getElementById("start-demo");
const stopDemo = document.getElementById("stop-demo");
const demoRoot = document.getElementById("demo-root");
const fallback = document.getElementById("demo-fallback");
const loadStatus = document.getElementById("demo-load-status");
let unmountDemo;
let demoStyle;
let failedDemoLoads = 0;
let demoState = "static";
startDemo.hidden = false;
const showFallback = (message) => {
  demoState = "static";
  demoRoot.hidden = true;
  fallback.hidden = false;
  stopDemo.hidden = true;
  startDemo.setAttribute("aria-disabled", "false");
  startDemo.textContent = "Explore the interactive demo";
  loadStatus.textContent = message;
};
const openDemo = async (focus = false) => {
  if (demoState !== "static") return;
  demoState = "loading";
  // Reserve the expanded scene before a slow bundle arrives.
  fallback.parentElement.dataset.demoOpen = "true";
  // Keep the control focusable while loading; demoState blocks repeat activation.
  startDemo.setAttribute("aria-disabled", "true");
  startDemo.textContent = "Opening the sample team…";
  loadStatus.textContent = "Loading the interactive sample. No AI tools are being connected.";
  try {
    const styleReady = new Promise((resolve, reject) => {
      if (demoStyle?.sheet) return resolve();
      demoStyle?.remove();
      demoStyle = document.createElement("link");
      demoStyle.rel = "stylesheet";
      demoStyle.href = startDemo.dataset.demoStyle;
      demoStyle.onload = resolve;
      demoStyle.onerror = reject;
      document.head.append(demoStyle);
    });
    // Browsers remember a failed module import. A retry gets a fresh URL for the
    // same local, content-hashed file instead of replaying that cached failure.
    const moduleUrl = new URL(startDemo.dataset.demoModule, window.location.href);
    if (failedDemoLoads) moduleUrl.searchParams.set("retry", String(failedDemoLoads));
    const [{ mountDemo }] = await Promise.all([import(moduleUrl.href), styleReady]);
    unmountDemo?.();
    demoRoot.hidden = false;
    unmountDemo = mountDemo(
      demoRoot,
      () => {
        demoState = "ready";
        const focusDemo = focus && document.activeElement === startDemo;
        fallback.hidden = true;
        stopDemo.hidden = false;
        if (focusDemo) demoRoot.querySelector('[role="tab"]')?.focus({ preventScroll: true });
      },
      () =>
        showFallback(
          "The interactive example could not open. You can still read the sample below.",
        ),
    );
  } catch {
    failedDemoLoads += 1;
    showFallback(
      "The interactive example could not load. You can retry, or read the sample below.",
    );
  }
};
startDemo.addEventListener("click", () => void openDemo(true));
stopDemo.addEventListener("click", () => {
  unmountDemo?.();
  unmountDemo = undefined;
  delete fallback.parentElement.dataset.demoOpen;
  showFallback("Illustrated sample. No real AI runs, sign-in, or saved changes.");
  startDemo.focus({ preventScroll: true });
});
void openDemo();
