import { describe, expect, it } from "vitest";

import { aiToolTitle, isBusyRefusal } from "./panel";

describe("an AI tool's sign-in tab (Phase 19)", () => {
  it("is named for what it runs", () => {
    expect(aiToolTitle("Codex", "signIn")).toBe("Sign in · Codex");
    expect(aiToolTitle("Kimi", "signOut")).toBe("Sign out · Kimi");
  });

  it("waits, then opens again, only while the AI tool is busy for now", () => {
    // A task is using it, or it is being updated: the card waits and tries again.
    expect(isBusyRefusal("1 task is using Codex. Plenipo waits until it finishes.")).toBe(true);
    expect(isBusyRefusal("2 tasks are using Codex. Plenipo waits until they finish.")).toBe(true);
    expect(isBusyRefusal("Codex is being updated. Plenipo waits until it's done.")).toBe(true);
    // Anything else is said in the tab, and nothing opens again by itself.
    expect(isBusyRefusal("Kimi has no sign-out command of its own.")).toBe(false);
    expect(
      isBusyRefusal("Codex's sign-in tab is open. Close it first; Plenipo waits until then."),
    ).toBe(false);
    expect(isBusyRefusal("A worker is using the screen.")).toBe(false);
  });
});
