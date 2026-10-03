import type { ChainOrder, HandoffView } from "@plenipo/types";
import { describe, expect, it } from "vitest";

import { chainLine, firstLine, listWords, standingOf } from "./plan";

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

describe("your orders in the chain of command (ADR-202)", () => {
  const order = (over: Partial<ChainOrder>): ChainOrder => ({
    taskId: "t1",
    at: 1,
    positionId: "p1",
    position: "Website Supervisor",
    via: null,
    words: "Fix the login page\nand tell me when",
    leads: ["Development Manager", "VP"],
    part: "doer",
    standing: "working",
    result: null,
    reportedAt: null,
    ...over,
  });

  it("says each part in plain words", () => {
    expect(chainLine(order({}))).toBe(
      "You asked it directly: “Fix the login page”. Plenipo told Development Manager and VP.",
    );
    expect(chainLine(order({ leads: [] }))).toBe("You asked it: “Fix the login page”.");
    expect(chainLine(order({ via: "Website Supervisor", position: "Developer" }))).toBe(
      "You asked it, through Website Supervisor: “Fix the login page”.",
    );
    expect(chainLine(order({ part: "via", position: "Developer" }))).toBe(
      "You asked its Developer, through it: “Fix the login page”.",
    );
    expect(chainLine(order({ part: "told" }))).toBe(
      "You asked Website Supervisor directly: “Fix the login page”.",
    );
    expect(
      chainLine(order({ part: "told", via: "Cloudline Supervisor", position: "Developer" })),
    ).toBe("You asked Developer through Cloudline Supervisor: “Fix the login page”.");
  });

  it("lists names as people say them", () => {
    expect(listWords(["A"])).toBe("A");
    expect(listWords(["A", "B"])).toBe("A and B");
    expect(listWords(["A", "B", "C"])).toBe("A, B, and C");
  });
});
