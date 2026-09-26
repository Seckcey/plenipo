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

describe("describeEvent (Phase 4 Liaison events)", () => {
  it("describes a handoff's whole trail in plain language", () => {
    expect(
      describeEvent(
        event("liaison.handoff_requested", {
          destination: "runtime:claude-code",
          objective: "Review the parser\nin detail",
        }),
      ),
    ).toBe("Handoff requested → claude-code: Review the parser");
    expect(
      describeEvent(event("liaison.handoff_received", { depth: 1, objective: "Review it" })),
    ).toBe("Received as a handoff (depth 1): Review it");
    expect(describeEvent(event("liaison.dispatched", {}))).toBe("Handoff worker started");
    expect(
      describeEvent(event("liaison.reply_sent", { outcome: "completed", summary: "Looks right" })),
    ).toBe("Reply sent: Completed — Looks right");
    expect(
      describeEvent(event("liaison.reply_received", { outcome: "rejected", summary: "No such" })),
    ).toBe("Reply received: Refused by Liaison — No such");
    expect(describeEvent(event("liaison.replies_delivered", { messageIds: ["a", "b"] }))).toBe(
      "Continued with 2 handoff replies",
    );
    expect(describeEvent(event("liaison.replies_delivered", { messageIds: ["a"] }))).toBe(
      "Continued with 1 handoff reply",
    );
  });

  it("explains refusals, cancellations, and failures with their reason", () => {
    expect(
      describeEvent(
        event("liaison.handoff_rejected", { reason: "the handoff depth limit (3) is reached" }),
      ),
    ).toBe("Handoff refused: the handoff depth limit (3) is reached");
    expect(
      describeEvent(event("liaison.handoff_cancelled", { reason: "the requesting task failed" })),
    ).toBe("Handoff cancelled: the requesting task failed");
    expect(
      describeEvent(event("liaison.dispatch_failed", { reason: "Codex is not available." })),
    ).toBe("Handoff worker could not start: Codex is not available.");
    expect(describeEvent(event("liaison.duplicate_ignored", {}))).toBe(
      "Duplicate handoff request ignored",
    );
    expect(
      describeEvent(event("liaison.reply_discarded", { messageIds: ["a"], reason: "stopped" })),
    ).toBe("Handoff reply discarded: stopped");
    expect(describeEvent(event("liaison.reply_refused", { reason: "wrong workflow" }))).toBe(
      "Reply refused: wrong workflow",
    );
  });
});

describe("describeEvent (Phase 5 organization events)", () => {
  it("describes changes to the organization and its workers", () => {
    expect(describeEvent(event("org.position_created", { title: "QA Engineer" }))).toBe(
      "Position created: QA Engineer",
    );
    expect(
      describeEvent(
        event("org.oversight_assigned", {
          kind: "security",
          overseer: "Security Auditor",
          target: "Website Coordinator",
        }),
      ),
    ).toBe("Security Auditor is now the security auditor for Website Coordinator's team");
    expect(describeEvent(event("org.worker_spawned", { title: "Senior Developer" }))).toBe(
      "Worker spawned for Senior Developer",
    );
    expect(describeEvent(event("org.worker_retired", { lifecycle: "failed" }))).toBe(
      "Worker failed and left the organization",
    );
    expect(describeEvent(event("org.position_moved", { title: "Designer", to: null }))).toBe(
      "Designer now reports to the owner",
    );
    expect(describeEvent(event("org.project_archived", { name: "Q4 Campaign" }))).toBe(
      "Project archived with its team: Q4 Campaign",
    );
  });
});
