// An accessibility smoke check (the plan's "accessibility smoke tests", Phase 12): the plain
// problems a screen reader or keyboard user would meet first. Not a full audit.

function isHidden(el: Element): boolean {
  return el.closest("[hidden], [aria-hidden='true']") !== null;
}

/** An element's accessible name, roughly as a screen reader computes it. */
export function nameOf(el: Element): string {
  const doc = el.ownerDocument;
  const label = el.getAttribute("aria-label")?.trim();
  if (label) return label;
  const by = el.getAttribute("aria-labelledby");
  if (by) {
    const named = by
      .split(/\s+/)
      .map((id) => doc.getElementById(id)?.textContent ?? "")
      .join(" ")
      .trim();
    if (named) return named;
  }
  if (
    el instanceof HTMLInputElement ||
    el instanceof HTMLSelectElement ||
    el instanceof HTMLTextAreaElement
  ) {
    const labels = [...(el.labels ?? [])]
      .map((l) => l.textContent ?? "")
      .join(" ")
      .trim();
    if (labels) return labels;
  }
  if (el instanceof HTMLImageElement) return el.alt.trim();
  return (el.textContent ?? "").trim() || (el.getAttribute("title") ?? "").trim();
}

const CONTROLS = [
  "button",
  "a[href]",
  "[role=button]",
  "[role=tab]",
  "[role=switch]",
  "[role=menuitem]",
  "[role=separator][tabindex]",
  "input:not([type=hidden])",
  "select",
  "textarea",
].join(", ");

/** The problems found under `root`: an empty list is a pass. */
export function a11yProblems(root: HTMLElement): string[] {
  const problems: string[] = [];
  const short = (el: Element) => el.outerHTML.replace(/\s+/g, " ").slice(0, 140);

  // Every control has a name.
  for (const el of root.querySelectorAll(CONTROLS)) {
    if (!isHidden(el) && !nameOf(el)) problems.push(`a control without a name: ${short(el)}`);
  }
  // Every picture says what it is, or says it is decoration (alt="").
  for (const img of root.querySelectorAll("img")) {
    if (!img.hasAttribute("alt")) problems.push(`a picture without alt: ${short(img)}`);
  }
  for (const el of root.querySelectorAll('[role="img"]')) {
    if (!isHidden(el) && !nameOf(el)) problems.push(`an image without a name: ${short(el)}`);
  }
  // One main heading, and no level skipped on the way down.
  const headings = [...root.querySelectorAll("h1, h2, h3, h4, h5, h6")].filter((h) => !isHidden(h));
  const mains = headings.filter((h) => h.tagName === "H1");
  if (mains.length !== 1) problems.push(`${mains.length} main headings (h1), not 1`);
  let last = 0;
  for (const h of headings) {
    const level = Number(h.tagName[1]);
    if (last && level > last + 1) {
      problems.push(`"${h.textContent?.trim()}" (${h.tagName}) skips a level after h${last}`);
    }
    last = level;
  }
  // Lists and regions that are labelled say what they are.
  for (const el of root.querySelectorAll(
    "section[aria-labelledby], [role=region], [role=tabpanel]",
  )) {
    if (!isHidden(el) && !nameOf(el)) problems.push(`a region without a name: ${short(el)}`);
  }
  // IDs are unique, and every reference to one points somewhere.
  const ids = new Map<string, number>();
  for (const el of root.querySelectorAll("[id]")) ids.set(el.id, (ids.get(el.id) ?? 0) + 1);
  for (const [id, n] of ids) if (n > 1) problems.push(`the id "${id}" is used ${n} times`);
  for (const attr of ["aria-labelledby", "aria-controls", "aria-describedby"]) {
    for (const el of root.querySelectorAll(`[${attr}]`)) {
      for (const id of (el.getAttribute(attr) ?? "").split(/\s+/).filter(Boolean)) {
        if (!root.ownerDocument.getElementById(id)) {
          problems.push(`${attr}="${id}" points nowhere: ${short(el)}`);
        }
      }
    }
  }
  return problems;
}
