import { describe, expect, it } from "vitest";

import { Newest } from "./newest";

describe("Newest", () => {
  it("lets replies in when they come back in the order their reloads started", () => {
    const order = new Newest();
    const first = order.start();
    const second = order.start();
    expect(first.take()).toBe(true);
    expect(second.take()).toBe(true);
  });

  it("drops an older reload's reply that comes back after a newer one", () => {
    const order = new Newest();
    const older = order.start();
    const newer = order.start();
    expect(newer.take()).toBe(true);
    expect(older.take()).toBe(false);
  });

  it("drops a reply from a reload that started before a change was applied", () => {
    const order = new Newest();
    const before = order.start();
    order.applied();
    expect(before.take()).toBe(false);
    // A reload that starts after the change is newer than it.
    expect(order.start().take()).toBe(true);
  });

  it("does not let an older failure show after a newer reply", () => {
    const order = new Newest();
    const older = order.start();
    const newer = order.start();
    expect(newer.take()).toBe(true);
    expect(older.fresh()).toBe(false);
  });

  it("still shows an older reply that comes back after a newer failure", () => {
    const order = new Newest();
    const older = order.start();
    const newer = order.start();
    // The newer reload fails first: its failure shows, but takes nothing.
    expect(newer.fresh()).toBe(true);
    // The older reply then comes: it shows (and the page clears the failure).
    expect(older.take()).toBe(true);
  });
});
