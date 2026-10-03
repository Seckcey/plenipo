import { describe, expect, it } from "vitest";

import { describeCommunityEvent } from "./words";

describe("Community in Activity", () => {
  it("says each event in plain words", () => {
    expect(
      describeCommunityEvent("community.signed_in", {
        account: "Frank Gonzalez",
        pc: "FRANKIE-DESKTOP",
      }),
    ).toBe("Signed in to Community as Frank Gonzalez on FRANKIE-DESKTOP");
    expect(describeCommunityEvent("community.joined", { name: "pat-lee" })).toBe(
      "Joined Community as @pat-lee",
    );
    expect(describeCommunityEvent("community.signed_out", { why: "you" })).toMatch(
      /^You signed this (PC|Mac|computer) out of Community$/,
    );
    expect(describeCommunityEvent("community.signed_out", { why: "removed" })).toContain(
      "removed from Community on the account site",
    );
    expect(describeCommunityEvent("community.signed_out", { why: "too_young" })).toContain(
      "13 and older",
    );
    expect(describeCommunityEvent("community.left", {})).toBe("You left Community");
    expect(
      describeCommunityEvent("community.profile_changed", {
        shown: ["picture", "name", "business", "region"],
      }),
    ).toBe(
      "You changed your Community profile. People see your picture, name, what your business does, and where",
    );
    expect(describeCommunityEvent("community.profile_changed", { shown: ["name"] })).toBe(
      "You changed your Community profile. People see your name",
    );
    expect(describeCommunityEvent("community.profile_changed", { shown: [] })).toBe(
      "You changed your Community profile. People see none of it",
    );
    // A part this copy doesn't know is left out, never shown as it was written.
    expect(
      describeCommunityEvent("community.profile_changed", {
        shown: ["<b>x</b>", "constructor", "mood"],
      }),
    ).toBe("You changed your Community profile. People see your mood");
    expect(describeCommunityEvent("community.appeared_offline", {})).toBe(
      "You chose Appear offline in Community",
    );
    expect(describeCommunityEvent("community.appeared_online", {})).toBe(
      "You turned off Appear offline in Community",
    );
    expect(describeCommunityEvent("remote.signed_in", {})).toBeNull();
  });

  it("never lets a name disguise itself", () => {
    const line = describeCommunityEvent("community.signed_in", {
      account: "Frank\u{202E}evil",
      pc: "PC",
    });
    expect(line).not.toContain("\u{202E}");
  });
});
