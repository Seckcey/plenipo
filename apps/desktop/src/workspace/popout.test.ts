import { afterEach, describe, expect, it } from "vitest";

import { dressWindow } from "./popout";

describe("a pop-out's page (ADR-092 §8, ADR-203)", () => {
  const added: Element[] = [];
  const addToHead = <T extends Element>(el: T): T => {
    document.head.appendChild(el);
    added.push(el);
    return el;
  };
  afterEach(() => {
    for (const el of added) el.remove();
    added.length = 0;
  });

  it("takes Plenipo's styles and never a script, now or later", async () => {
    addToHead(Object.assign(document.createElement("style"), { textContent: ".kept{}" }));
    addToHead(Object.assign(document.createElement("script"), { textContent: "window.x = 1" }));
    const doc = document.implementation.createHTMLDocument("");
    const { stop } = dressWindow(doc, "Plenipo · Developer");
    expect(doc.title).toBe("Plenipo · Developer");
    expect([...doc.head.querySelectorAll("style")].some((s) => s.textContent === ".kept{}")).toBe(
      true,
    );
    expect(doc.querySelectorAll("script")).toHaveLength(0);

    // Added later (a part loaded when first needed): a style follows, a script never does.
    addToHead(Object.assign(document.createElement("script"), { textContent: "window.y = 1" }));
    addToHead(Object.assign(document.createElement("link"), { rel: "stylesheet", href: "/a.css" }));
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(doc.querySelectorAll("script")).toHaveLength(0);
    expect(doc.head.querySelector('link[rel="stylesheet"]')).not.toBeNull();
    stop();
  });
});
