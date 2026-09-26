import { describe, expect, it } from "vitest";

import { samplePermissions, T0 } from "../test/permissionFixtures";
import { CAPABILITIES, capabilityLabel, setSummary, timeLeft } from "./format";

describe("permission words", () => {
  it("names every capability of the plan", () => {
    expect(CAPABILITIES).toHaveLength(16);
    expect(capabilityLabel("git.write")).toBe("Save to git");
    expect(capabilityLabel("something.new")).toBe("something.new");
  });

  it("summarizes a permission set", () => {
    const [readOnly] = samplePermissions().settings.sets;
    expect(setSummary(readOnly!)).toBe("Read files, Read git history");
    expect(setSummary({ ...readOnly!, levels: {} })).toBe("Nothing (conversation only)");
  });

  it("says how long an approval still waits", () => {
    expect(timeLeft(T0 + 9 * 60_000, T0)).toBe("9 min left");
    expect(timeLeft(T0 + 45_000, T0)).toBe("45 s left");
    expect(timeLeft(T0 - 1, T0)).toBe("time is up");
    expect(timeLeft(null, T0)).toBe("");
  });
});
