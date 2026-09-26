import { describe, expect, it } from "vitest";

import {
  RANKS,
  TITLE_SETS,
  chainOf,
  rankName,
  roleLabel,
  titleSet,
  titlesOf,
  withArticle,
} from "./titles";

describe("titles", () => {
  it("defaults to the plain business chain", () => {
    expect(chainOf(titleSet("business"))).toBe("President → VP → Manager → Supervisor → Worker");
    expect(titlesOf({ titles: "business" }).label).toBe("Business");
    expect(titleSet(null).theme).toBe("business");
    expect(titleSet("starfleet" as never).theme).toBe("business");
  });

  it("offers each U.S. branch and the Mafia, with a name for every rank", () => {
    expect(TITLE_SETS.map((t) => t.label)).toEqual([
      "Business",
      "U.S. Army",
      "U.S. Navy",
      "U.S. Air Force",
      "U.S. Marine Corps",
      "U.S. Coast Guard",
      "U.S. Space Force",
      "Mafia",
    ]);
    expect(new Set(TITLE_SETS.map((t) => t.theme)).size).toBe(TITLE_SETS.length);
    for (const t of TITLE_SETS) {
      for (const rank of RANKS) {
        expect(t.ranks[rank].one.trim(), `${t.theme} ${rank}`).not.toBe("");
        expect(t.ranks[rank].many.trim(), `${t.theme} ${rank}`).not.toBe("");
      }
    }
    expect(chainOf(titleSet("army"))).toBe("General → Colonel → Captain → Sergeant → Private");
    expect(chainOf(titleSet("navy"))).toBe(
      "Admiral → Captain → Lieutenant → Chief Petty Officer → Seaman",
    );
    expect(chainOf(titleSet("mafia"))).toBe("Don → Underboss → Capo → Soldier → Associate");
  });

  it("names ranks with plurals and articles", () => {
    const business = titleSet("business");
    expect(rankName(business, "superintendent", 2)).toBe("VPs");
    expect(rankName(titleSet("airForce"), "worker", 3)).toBe("Airmen");
    expect(rankName(titleSet("mafia"), "superintendent", 2)).toBe("Underbosses");
    expect(withArticle("Supervisor")).toBe("a Supervisor");
    expect(withArticle("Underboss")).toBe("an Underboss");
    expect(withArticle("VP")).toBe("a VP");
  });

  it("calls leadership templates by their rank, and other roles by name", () => {
    const army = titleSet("army");
    expect(roleLabel(army, { name: "VP", kind: "superintendent", template: true })).toBe("Colonel");
    expect(
      roleLabel(army, { name: "Chief of Staff", kind: "superintendent", template: false }),
    ).toBe("Chief of Staff");
    expect(roleLabel(army, { name: "Senior Developer", kind: "worker", template: true })).toBe(
      "Senior Developer",
    );
  });
});
