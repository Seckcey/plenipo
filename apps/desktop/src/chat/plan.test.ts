import type { HandoffView } from "@plenipo/types";
import { describe, expect, it } from "vitest";

import { firstLine, standingOf } from "./plan";

function handoff(over: Partial<HandoffView>): HandoffView {
  return {
    messageId: "m1",
    correlationId: "c1",
    state: "dispatched",
    requesterTaskId: "t1",
    requester: "session:s1",
    requesterRuntimeId: null,
    step: 1,
    destination: "role:Developer",
    destinationLabel: "Developer",
    objective: "Write the script",
    acceptanceCriteria: "",
    priority: 0,
    depth: 1,
    context: [],
    artifacts: [],
    capabilitiesRequested: [],
    rejection: null,
    childTaskId: "t2",
    childSessionId: "s2",
    childState: "queued",
    reply: null,
    createdAt: 1,
    updatedAt: 1,
    ...over,
  };
}

describe("where a team task stands", () => {
  it("follows its worker's task", () => {
    expect(standingOf(handoff({ childState: "queued" }))).toBe("waiting");
    expect(standingOf(handoff({ childState: "running" }))).toBe("working");
    expect(standingOf(handoff({ childState: "succeeded" }))).toBe("done");
    expect(standingOf(handoff({ childState: "failed" }))).toBe("failed");
    expect(standingOf(handoff({ childState: "cancelled" }))).toBe("stopped");
    expect(standingOf(handoff({ childState: null, state: "answered" }))).toBe("done");
    expect(standingOf(handoff({ state: "rejected" }))).toBe("failed");
    expect(standingOf(handoff({ state: "cancelled", childState: "running" }))).toBe("stopped");
  });

  it("shows a task's first line, short", () => {
    expect(firstLine("\n  Write the script  \nthen test it")).toBe("Write the script");
    expect(firstLine("x".repeat(200))).toHaveLength(140);
  });
});
