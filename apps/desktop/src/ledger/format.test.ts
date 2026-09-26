import type { LedgerEvent } from "@plenipo/types";
import { describe, expect, it } from "vitest";

import { describeEvent } from "./format";

const event = (eventType: string, payload: Record<string, unknown>): LedgerEvent => ({
  seq: 1,
  id: "e",
  taskId: "t",
  executionId: null,
  source: "agent:claude-code",
  destination: null,
  eventType,
  payload,
  createdAt: 0,
});

describe("describeEvent (Phase 3 agent events)", () => {
  it("describes agent activity and results in plain language", () => {
    expect(
      describeEvent(event("agent.session_bound", { providerSessionId: "p-1", model: "m" })),
    ).toBe("Provider session p-1 · model m");
    expect(describeEvent(event("agent.message", { text: "\nHello there\nsecond line" }))).toBe(
      "Agent: Hello there",
    );
    expect(describeEvent(event("agent.tool_use", { tool: "shell", summary: "ls" }))).toBe(
      "Tool shell: ls",
    );
    expect(
      describeEvent(
        event("agent.tool_result", { tool: "shell", isError: true, summary: "exit 1" }),
      ),
    ).toBe("Tool result (error): exit 1");
    expect(
      describeEvent(event("agent.result", { outcome: "usageLimited", summary: "Try later" })),
    ).toBe("Result: Usage limit reached — Try later");
    expect(describeEvent(event("session.opened", { runtime: "codex" }))).toBe(
      "Worker session opened on codex",
    );
    expect(describeEvent(event("agent.message", { text: "x".repeat(500) })).length).toBeLessThan(
      170,
    );
  });
});
