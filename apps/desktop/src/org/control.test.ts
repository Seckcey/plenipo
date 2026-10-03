import { describe, expect, it } from "vitest";

import { experienceLine } from "./control";

describe("experienceLine", () => {
  it("counts kept lessons and finished tasks", () => {
    expect(experienceLine({ score: 57, keptLessons: 4, tasksDone: 17, experienced: true })).toBe(
      "57 — 4 lessons you kept, 17 tasks done",
    );
  });

  // Phase 25, item 4.8: repeat failures show on its Experience.
  it("says how many of its answers didn't match Plenipo's record", () => {
    const e = { score: 12, keptLessons: 1, tasksDone: 2, experienced: false };
    expect(experienceLine({ ...e, answersSentBack: 1 })).toBe(
      "12 — 1 lesson you kept, 2 tasks done; 1 answer didn't match the record",
    );
    expect(experienceLine({ ...e, answersSentBack: 3 })).toBe(
      "12 — 1 lesson you kept, 2 tasks done; 3 answers didn't match the record",
    );
  });
});
