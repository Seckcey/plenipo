import type { HandoffView } from "@plenipo/types";
import { describe, expect, it } from "vitest";

import { matchRequests, splitHandoffs } from "./handoffBlocks";

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

  it("pairs each request with its record by its words and its worker, so like requests keep their cards", () => {
    const view = (messageId: string, destination: string, objective: string) =>
      ({ messageId, destination, objective }) as HandoffView;
    // The same words to two workers, recorded in the other order.
    const asked = [
      { to: "codex", objective: "Review it" },
      { to: "claude-code", objective: "Review it" },
      { to: "role:QA Engineer", objective: "Run the tests" },
    ];
    const sent = [
      view("h-claude", "claude-code", "Review it"),
      view("h-codex", "codex", "Review it"),
      view("h-qa", "role:QA Engineer", "Run the tests"),
    ];
    expect(matchRequests(asked, sent).map((v) => v?.messageId)).toEqual([
      "h-codex",
      "h-claude",
      "h-qa",
    ]);
    // Where the worker was recorded differently, its words alone still find it; one with no
    // record has no card to pair with.
    const other = [view("h-1", "runtime:codex", "Review it")];
    expect(matchRequests(asked, other).map((v) => v?.messageId ?? null)).toEqual([
      "h-1",
      null,
      null,
    ]);
  });
});
