import type { TaskCost } from "@plenipo/types";
import { describe, expect, it } from "vitest";

import { moneyWords, pricedWords, tokensDetail, tokensWords } from "./costWords";

const cost = (patch: Partial<TaskCost> = {}): TaskCost => ({
  taskId: "t1",
  read: 9_000,
  reused: 7_000,
  written: 3_000,
  runs: 2,
  counted: 2,
  atLeast: false,
  spentMicros: 0,
  setAsideMicros: 0,
  notPriced: 0,
  notPricedMicros: 0,
  pricedBy: null,
  running: false,
  ...patch,
});

describe("a task's cost in words (I2)", () => {
  it("says its tokens, at least when a run stopped before it reported, and none when none came", () => {
    expect(tokensWords(cost())).toBe("12K tokens");
    expect(tokensWords(cost({ atLeast: true }))).toBe("at least 12K tokens");
    expect(tokensWords(cost({ counted: 0, read: 0, written: 0 }))).toBeNull();
    expect(tokensDetail(cost())).toBe("9,000 read (7,000 reused) · 3,000 written");
    expect(tokensDetail(cost({ reused: 0 }))).toBe("9,000 read · 3,000 written");
  });

  it("says money only for paid keys, with how it was priced, and so far while one runs", () => {
    // A subscription's tools: tokens only.
    expect(moneyWords(cost())).toBeNull();
    expect(pricedWords(cost())).toBeNull();
    const paid = cost({ spentMicros: 40_000, pricedBy: "service" });
    expect(moneyWords(paid)).toBe("$0.04");
    expect(pricedWords(paid)).toBe("spent $0.04, priced by the AI company's own bill");
    const running = cost({ spentMicros: 40_000, pricedBy: "priceList", setAsideMicros: 500_000 });
    expect(moneyWords(running)).toBe("$0.04 so far");
    expect(pricedWords(running)).toBe(
      "spent $0.04, priced from Plenipo's price list · still running: up to $0.50 set aside",
    );
    const unpriced = cost({ notPriced: 1, notPricedMicros: 250_000 });
    expect(moneyWords(unpriced)).toBe("$0.25");
    expect(pricedWords(unpriced)).toBe("not priced yet: counted as $0.25");
  });
});
