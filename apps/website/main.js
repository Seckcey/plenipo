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
