import { describe, expect, it } from "vitest";

import { splitHandoffs } from "./handoffBlocks";

const block = (json: string) => "```plenipo-handoff\n" + json + "\n```";

describe("a lead's handoff requests in its words (B5)", () => {
  it("takes each closed request out of the words, in order", () => {
    const text = [
      "I'll get a review.",
      block('{"to": "claude-code", "objective": "Review the answer above"}'),
      "And a test run.",
      block('{"to": "role:QA Engineer", "objective": "Run the tests", "context": []}'),
    ].join("\n");
    expect(splitHandoffs(text)).toEqual([
      { kind: "text", text: "I'll get a review.\n" },
      { kind: "handoff", to: "claude-code", objective: "Review the answer above" },
      { kind: "text", text: "\nAnd a test run.\n" },
      { kind: "handoff", to: "role:QA Engineer", objective: "Run the tests" },
    ]);
  });

  it("leaves words alone: no request, one still being written, or one that is not a request", () => {
    expect(splitHandoffs("Just words.")).toEqual([{ kind: "text", text: "Just words." }]);
    const writing = 'Asking now.\n```plenipo-handoff\n{"to": "codex", "obj';
    expect(splitHandoffs(writing)).toEqual([{ kind: "text", text: writing }]);
    const notJson = block("not json");
    expect(splitHandoffs(notJson)).toEqual([{ kind: "text", text: notJson }]);
    const noDestination = block('{"objective": "Do it"}');
    expect(splitHandoffs(noDestination)).toEqual([{ kind: "text", text: noDestination }]);
  });
});
