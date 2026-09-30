/**
 * A popped-out panel's window (Phase 21, ADR-092 §8). The organization's window opens it empty
 * (`about:blank`, allowed by Plenipo only right after the page asked) and draws the panel into it
 * itself, so the panel is the same one, not a copy. This makes the empty window look like
 * Plenipo: its styles, its light or dark theme (followed as it changes), and its title.
 */

/** How long to wait for the new window's page to be ready. */
const READY_MS = 3000;

/** Wait until a window just opened has a page to draw into (`null` if it never does). */
export async function whenReady(win: Window): Promise<Document | null> {
  const started = Date.now();
  while (Date.now() - started < READY_MS) {
    if (win.closed) return null;
    try {
      const doc = win.document;
      if (doc?.body && doc.readyState !== "loading") return doc;
    } catch {
      // Not reachable yet.
    }
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  return null;
}

function copyStyle(node: Node, into: Document): Node | null {
  if (node instanceof HTMLLinkElement && node.rel === "stylesheet") {
    const link = into.createElement("link");
    link.rel = "stylesheet";
    // The full address: the new window's own page has none to start from.
    link.href = node.href;
    return link;
  }
  if (node instanceof HTMLStyleElement) {
    const style = into.createElement("style");
    style.textContent = node.textContent;
    return style;
  }
  return null;
}

function copyAttributes(from: Element, to: Element) {
  for (const name of to.getAttributeNames()) {
    if (!from.hasAttribute(name)) to.removeAttribute(name);
  }
  for (const attr of Array.from(from.attributes)) {
    if (to.getAttribute(attr.name) !== attr.value) to.setAttribute(attr.name, attr.value);
  }
}

/**
 * Dress a new window's page like Plenipo's and give it a place for the panel. Returns that place
 * and a function that stops following Plenipo's styles and theme.
 */
export function dressWindow(doc: Document, title: string): { root: HTMLElement; stop: () => void } {
  doc.title = title;
  const source = document;
  copyAttributes(source.documentElement, doc.documentElement);
  const copies = new Map<Node, Node>();
  const add = (node: Node) => {
    if (copies.has(node)) return;
    const copy = copyStyle(node, doc);
    if (!copy) return;
    copies.set(node, copy);
    doc.head.appendChild(copy);
  };
  source.head.childNodes.forEach(add);
  // Styles added later (a part loaded when first needed) and the theme follow.
  const heads = new MutationObserver((changes) => {
    for (const change of changes) {
      change.addedNodes.forEach(add);
      change.removedNodes.forEach((node) => {
        const copy = copies.get(node);
        if (copy?.parentNode) copy.parentNode.removeChild(copy);
        copies.delete(node);
      });
    }
  });
  heads.observe(source.head, { childList: true });
  const theme = new MutationObserver(() =>
    copyAttributes(source.documentElement, doc.documentElement),
  );
  theme.observe(source.documentElement, { attributes: true });
  doc.body.className = "popout";
  const root = doc.createElement("div");
  root.className = "popout__root";
  doc.body.replaceChildren(root);
  return {
    root,
    stop: () => {
      heads.disconnect();
      theme.disconnect();
    },
  };
}

/**
 * Where to open a panel dropped past the window's edge: its top left a little before the pointer,
 * the size it had, in the screen's own pixels (logical: the page's).
 */
export function dropPlace(
  screenX: number,
  screenY: number,
  size: { width: number; height: number },
) {
  return {
    x: Math.round(screenX - 60),
    y: Math.round(screenY - 16),
    width: Math.round(Math.max(480, size.width)),
    height: Math.round(Math.max(300, size.height)),
  };
}

/** The pointer is past this window's edges. */
export function outsideWindow(win: Window, clientX: number, clientY: number): boolean {
  return clientX < 0 || clientY < 0 || clientX >= win.innerWidth || clientY >= win.innerHeight;
}

/** A point on the screen is inside a window (its page, give or take its frame). */
export function insideWindow(win: Window, screenX: number, screenY: number): boolean {
  return (
    screenX >= win.screenX &&
    screenY >= win.screenY &&
    screenX < win.screenX + win.outerWidth &&
    screenY < win.screenY + win.outerHeight
  );
}
