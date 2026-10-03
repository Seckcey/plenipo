import { describe, expect, it } from "vitest";

import { sampleRouting } from "../../test/routingFixtures";
import { neverUsed, neverUsedWords } from "./neverUsed";

// Phase 25, item 1.6: next to every model picker, the AI companies never used there, and where
// each entry comes from.
describe("neverUsed", () => {
  it("adds up the organization's list and any kept from before, saying where each is from", () => {
    const base = sampleRouting();
    const snapshot = {
      ...base,
      companies: [
        { id: "deepseek", label: "DeepSeek" },
        { id: "xai", label: "xAI" },
        { id: "openai", label: "OpenAI" },
      ],
      organization: { ...base.organization, neverCompanies: ["deepseek"] },
      departments: [
        {
          departmentId: "d1",
          name: "Development",
          rule: { ...base.organization, neverCompanies: ["xai", "deepseek"] },
        },
      ],
    };
    const list = neverUsed(snapshot, { departmentId: "d1" });
    expect(list).toEqual([
      { company: "DeepSeek", from: "the whole organization" },
      { company: "xAI", from: "the Development department" },
    ]);
    expect(neverUsedWords(list)).toBe(
      "Never used here: DeepSeek (the whole organization), xAI (the Development department).",
    );
    expect(neverUsedWords(neverUsed(snapshot, {}))).toBe(
      "Never used here: DeepSeek (the whole organization).",
    );
    expect(neverUsedWords([])).toBeNull();
  });
});
