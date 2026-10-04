import { describe, expect, it } from "vitest";

import { Newest } from "./newest";

describe("Newest", () => {
  it("lets replies in when they come back in the order their reloads started", () => {
    const order = new Newest();
    const first = order.start();
    const second = order.start();
    expect(first()).toBe(true);
    expect(second()).toBe(true);
  });

  it("drops an older reload's reply that comes back after a newer one", () => {
    const order = new Newest();
    const older = order.start();
    const newer = order.start();
    expect(newer()).toBe(true);
    expect(older()).toBe(false);
  });

  it("drops a reply from a reload that started before a change was applied", () => {
    const order = new Newest();
    const before = order.start();
    order.applied();
    expect(before()).toBe(false);
    // A reload that starts after the change is newer than it.
    expect(order.start()()).toBe(true);
  });

  it("counts a newer failure as the newest look, so an older reply cannot follow it", () => {
    const order = new Newest();
    const older = order.start();
    const failed = order.start();
    expect(failed()).toBe(true);
    expect(older()).toBe(false);
  });
});
