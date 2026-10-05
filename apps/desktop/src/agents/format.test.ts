import { describe, expect, it } from "vitest";

import { turn } from "../test/agentFixtures";
import {
  describePrompt,
  describeUsage,
  turnPromptSizes,
  turnUsage,
  type PromptSize,
} from "./format";

const size = (patch: Partial<PromptSize> = {}): PromptSize => ({
  bytes: 900,
  ownBytes: 410,
  brief: "reminder",
  why: "routine",
  fullOwnBytes: 6_600,
  note: "reminder",
  ...patch,
});

const result = (prompt?: PromptSize) => ({
  outcome: "completed" as const,
  summary: "Done",
  text: "Done",
  error: null,
  providerSessionId: null,
  model: null,
  usage: null,
  durationMs: null,
  ignoredLines: 0,
  ...(prompt ? { prompt } : {}),
});

describe("Plenipo's own text with a task (ADR-044)", () => {
  it("names the size and what the first message carried", () => {
    expect(describePrompt([size()])).toBe("Plenipo's own text: 0.4 KB (a short reminder)");
    expect(describePrompt([size({ brief: "full", why: "first", ownBytes: 6_666 })])).toBe(
      "Plenipo's own text: 6.5 KB (full instructions)",
    );
  });

  it("adds up the steps of a task that got replies", () => {
    expect(describePrompt([size(), size({ brief: "replies", ownBytes: 1_100 })])).toBe(
      "Plenipo's own text: 1.5 KB (a short reminder, then replies)",
    );
  });

  it("says nothing when Plenipo added nothing of its own", () => {
    expect(describePrompt([])).toBeNull();
    expect(describePrompt([size({ brief: "plain", ownBytes: 0, note: "none" })])).toBeNull();
    // Only its permissions note, with an objective sent as it is.
    expect(describePrompt([size({ brief: "plain", ownBytes: 30 })])).toBe(
      "Plenipo's own text: 0.1 KB",
    );
  });

  it("reads each step's size, or the result's alone", () => {
    const steps = turn("t1", {
      steps: [
        {
          number: 1,
          executionId: "e1",
          running: false,
          result: result(size()),
          startedAt: 1,
          endedAt: 2,
        },
        {
          number: 2,
          executionId: "e2",
          running: true,
          result: null,
          startedAt: 3,
          endedAt: null,
          startedBy: { kind: "replies", messageIds: ["r1"] },
        },
      ],
    });
    expect(turnPromptSizes(steps)).toEqual([size()]);
    const older = turn("t1", { result: result(size({ brief: "full" })) });
    expect(turnPromptSizes(older)).toEqual([size({ brief: "full" })]);
    expect(turnPromptSizes(turn("t1", { result: result() }))).toEqual([]);
  });

  it("adds up the tokens over the same steps as the sizes", () => {
    const used = (inputTokens: number, outputTokens: number) => ({
      ...result(size()),
      usage: { inputTokens, cachedInputTokens: 0, outputTokens },
    });
    const replied = turn("t1", {
      steps: [
        {
          number: 1,
          executionId: "e1",
          running: false,
          result: used(100, 10),
          startedAt: 1,
          endedAt: 2,
        },
        {
          number: 2,
          executionId: "e2",
          running: false,
          result: used(50, 5),
          startedAt: 3,
          endedAt: 4,
        },
      ],
      result: used(50, 5),
    });
    expect(describeUsage(turnUsage(replied)!)).toBe("150 in · 15 out");
    // An older task: the result's own.
    expect(turnUsage(turn("t1", { result: used(7, 3) }))).toEqual({
      inputTokens: 7,
      cachedInputTokens: 0,
      outputTokens: 3,
    });
    expect(turnUsage(turn("t1", { result: result() }))).toBeNull();
  });
});
