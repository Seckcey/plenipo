import type { LedgerEvent } from "@plenipo/types";
import { describe, expect, it } from "vitest";

import { describeEvent, sourceLabel } from "./format";

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
    ).toBe("Conversation p-1 · model m");
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
      "Worker conversation opened on codex",
    );
    expect(sourceLabel("runtime", { capitalize: true })).toBe("Plenipo");
    expect(sourceLabel("owner")).toBe("you");
    expect(sourceLabel("owner", { capitalize: true })).toBe("You");
    expect(sourceLabel("agent:claude-code")).toBe("Claude Code");
    expect(sourceLabel("agent:gemini")).toBe("gemini");
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
      "Worker brought in for Senior Developer",
    );
    expect(describeEvent(event("org.worker_retired", { lifecycle: "failed" }))).toBe(
      "Worker failed and left the organization",
    );
    expect(describeEvent(event("org.position_moved", { title: "Designer", to: null }))).toBe(
      "Designer now reports to you",
    );
    expect(describeEvent(event("org.project_archived", { name: "Q4 Campaign" }))).toBe(
      "Project archived with its team: Q4 Campaign",
    );
  });
});

describe("describeEvent (Phase 6 routing)", () => {
  it("says which model a worker or agent got, and why", () => {
    const routing = {
      reason: "Opus (Claude Code) is Senior Developer's first choice and is ready.",
    };
    expect(describeEvent(event("org.worker_spawned", { title: "Senior Developer", routing }))).toBe(
      "Worker brought in for Senior Developer — Opus (Claude Code) is Senior Developer's first choice and is ready.",
    );
    expect(
      describeEvent(
        event("org.agent_routed", {
          title: "Website Supervisor",
          runtimeId: "codex",
          model: "gpt-x",
          routing: { reason: "Codex is Supervisor's first choice and is ready." },
        }),
      ),
    ).toBe(
      "Website Supervisor's agent starts its conversation on Codex · gpt-x — Codex is Supervisor's first choice and is ready.",
    );
    expect(describeEvent(event("router.policy_changed", { role: "Designer" }))).toBe(
      "Model choices changed for Designer",
    );
    expect(describeEvent(event("router.model_saved", { model: { label: "Opus" } }))).toBe(
      "Model saved: Opus",
    );
    expect(describeEvent(event("router.limit_cleared", { label: "Codex" }))).toBe(
      "You asked to try Codex again after its usage limit",
    );
  });
});

describe("describeEvent (Phase 7 permissions)", () => {
  it("describes permissions given, used, blocked, and revoked in plain words", () => {
    expect(
      describeEvent(
        event("guard.grant_opened", {
          worker: "Backend Developer",
          folder: "D:\\projects\\website",
          permissions: { "filesystem.read": "allowed", "powershell.exec": "ask" },
        }),
      ),
    ).toBe(
      "Permissions given to Backend Developer in D:\\projects\\website: Read files, Run PowerShell scripts (asks you)",
    );
    expect(
      describeEvent(
        event("capability.used", {
          worker: "Backend Developer",
          summary: "run cargo test",
          ok: false,
          result: "Failed with exit code 101\nmore",
        }),
      ),
    ).toBe("Backend Developer: run cargo test (failed) — Failed with exit code 101");
    expect(
      describeEvent(
        event("guard.denied", {
          worker: "Reviewer",
          summary: "write notes.txt",
          reason: "Blocked: the Reviewer set does not allow changing files.",
        }),
      ),
    ).toBe(
      "Blocked: Reviewer tried to write notes.txt — Blocked: the Reviewer set does not allow changing files.",
    );
    expect(
      describeEvent(
        event("guard.grant_closed", { worker: "Reviewer", used: 2, blocked: 1, asked: 0 }),
      ),
    ).toBe("Permissions ended for Reviewer: 2 done, 1 blocked, 0 asked you");
    expect(describeEvent(event("guard.grant_revoked", { worker: "Reviewer" }))).toBe(
      "You revoked Reviewer's permissions",
    );
    expect(sourceLabel("guard")).toBe("Guard");
  });

  it("describes approvals and settings", () => {
    expect(
      describeEvent(
        event("approval.requested", {
          actionType: "git.write",
          request: { summary: "git push origin" },
        }),
      ),
    ).toBe("Waiting for your approval: git push origin");
    expect(
      describeEvent(
        event("approval.resolved", {
          state: "approved",
          summary: "git push origin",
          note: "Approved by you.",
        }),
      ),
    ).toBe("Approved: git push origin");
    expect(
      describeEvent(
        event("approval.resolved", {
          state: "rejected",
          summary: "run x",
          note: "Permissions revoked.",
        }),
      ),
    ).toBe("Not approved: run x (Permissions revoked.)");
    expect(
      describeEvent(
        event("approval.expired", {
          actionType: "shell.exec",
          note: "No answer within 10 minute(s).",
        }),
      ),
    ).toBe("Approval expired: Run programs (No answer within 10 minute(s).)");
    expect(describeEvent(event("guard.set_added", { set: { name: "Docs" } }))).toBe(
      "Permission set added: Docs",
    );
    expect(describeEvent(event("guard.role_assigned", { role: "Code Reviewer" }))).toBe(
      "Code Reviewer's permissions changed",
    );
    expect(describeEvent(event("vault.secret_added", { name: "GitHub token" }))).toBe(
      "Secret stored: GitHub token",
    );
  });
});
