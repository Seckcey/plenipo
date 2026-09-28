import type { LedgerEvent } from "@plenipo/types";
import { describe, expect, it } from "vitest";

import { describeEvent, eventOutput, lasted, shownInTrail, sourceLabel } from "./format";

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
    // ADR-044: an AI tool that shortened its memory says so in plain words.
    expect(
      describeEvent(
        event("agent.memory_shortened", {
          detail:
            "Kimi shortened its memory of this conversation: it holds less of it than before.",
        }),
      ),
    ).toBe("Kimi shortened its memory of this conversation: it holds less of it than before.");
    expect(describeEvent(event("agent.memory_shortened", {}))).toBe(
      "The AI tool shortened its memory of this conversation",
    );
    expect(describeEvent(event("session.opened", { runtime: "codex" }))).toBe(
      "Worker conversation opened on codex",
    );
    expect(sourceLabel("runtime", { capitalize: true })).toBe("Plenipo");
    expect(sourceLabel("owner")).toBe("you");
    expect(sourceLabel("owner", { capitalize: true })).toBe("You");
    expect(sourceLabel("agent:claude-code")).toBe("Claude Code");
    expect(sourceLabel("agent:grok")).toBe("Grok");
    expect(sourceLabel("agent:kimi")).toBe("Kimi");
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
    // Ordinary queueing, in plain words (it was shown as its raw name before).
    expect(
      describeEvent(
        event("liaison.waiting_for_member", {
          reason: "waiting for Senior Developer to finish its current task",
        }),
      ),
    ).toBe("Waiting for Senior Developer to finish its current task");
    expect(describeEvent(event("liaison.waiting_for_member", {}))).toBe("Waiting its turn");
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
    expect(
      describeEvent(
        event("tool_server.ticket_refused", {
          worker: "Reviewer",
          connectingPid: 4242,
          expectedRootPid: 100,
        }),
      ),
    ).toBe(
      "Blocked: a program outside Reviewer's AI tool tried to use Reviewer's tools (program 4242; the AI tool is program 100)",
    );
    expect(describeEvent(event("tool_server.ticket_refused", { worker: "Reviewer" }))).toBe(
      "Blocked: a program outside Reviewer's AI tool tried to use Reviewer's tools (program unknown; the AI tool is program unknown)",
    );
    expect(describeEvent(event("tool_server.ticket_unchecked", { worker: "Reviewer" }))).toBe(
      "Plenipo could not check which program connected to Reviewer's tools on this computer, so it let it through",
    );
    expect(
      describeEvent(
        event("guard.approvals_limited", { worker: "Reviewer", limit: "waiting", waiting: 3 }),
      ),
    ).toBe(
      "Blocked: Reviewer asked for your approval again while 3 of its requests were waiting for you",
    );
    expect(
      describeEvent(event("guard.approvals_limited", { worker: "Reviewer", limit: "minute" })),
    ).toBe("Blocked: Reviewer asked for your approval too many times in one minute");
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

describe("describeEvent (Phase 10 browser and desktop events)", () => {
  it("says who used the browser or the mouse and keyboard, and what the owner did", () => {
    expect(describeEvent(event("browser.started", { browser: "Google Chrome" }))).toBe(
      "Plenipo's browser started (Google Chrome)",
    );
    expect(
      describeEvent(
        event("browser.tab_stopped", {
          worker: "Web Assistant",
          why: "the page kept removing Plenipo's sign that a worker is using the browser",
        }),
      ),
    ).toBe(
      "Plenipo stopped Web Assistant's use of the browser (the page kept removing Plenipo's sign that a worker is using the browser)",
    );
    expect(
      describeEvent(event("control.started", { kind: "browser", worker: "Web Assistant" })),
    ).toBe("Web Assistant started using Plenipo's browser");
    expect(
      describeEvent(
        event("control.started", {
          kind: "desktop",
          worker: "Operator",
          reason: "The app has no API",
        }),
      ),
    ).toBe("Operator started using the mouse and keyboard — The app has no API");
    expect(
      describeEvent(event("control.taken_over", { kind: "browser", worker: "Web Assistant" })),
    ).toBe("You took over Plenipo's browser from Web Assistant");
    expect(
      describeEvent(event("control.stopped", { sessions: [{ kind: "browser", worker: "W" }] })),
    ).toBe("You pressed Stop: 1 worker stopped using the browser, the desktop, or servers");
    expect(describeEvent(event("control.stopped", { sessions: [] }))).toBe(
      "You pressed Stop: browser, desktop, and server work is stopped",
    );
    expect(describeEvent(event("control.allowed", {}))).toBe(
      "You allowed browser, desktop, and server work again",
    );
    expect(describeEvent(event("guard.websites_changed", {}))).toBe("Website lists changed");
    expect(describeEvent(event("guard.switches_changed", {}))).toBe(
      "Switches changed (Settings → Switches)",
    );
    expect(describeEvent(event("guard.browser_chosen", { browserChoice: "chrome" }))).toBe(
      "Plenipo's browser set to Google Chrome",
    );
    expect(describeEvent(event("guard.browser_chosen", { browserChoice: "automatic" }))).toBe(
      "Plenipo's browser set to Automatic (Microsoft Edge, or Google Chrome without it)",
    );
    expect(
      describeEvent(
        event("control.switched_off", { kind: "browser", sessions: [{ worker: "W" }] }),
      ),
    ).toBe("You switched Plenipo's browser off: 1 worker stopped");
    expect(describeEvent(event("guard.sets_updated", { sets: ["writer"] }))).toBe(
      "Built-in permission sets you had not changed were brought up to date",
    );
    expect(
      describeEvent(event("org.role_updated", { name: "Scout", formerly: "Researcher 2" })),
    ).toBe("Role renamed from Researcher 2 to Scout");
    expect(describeEvent(event("org.role_updated", { name: "Designer", template: true }))).toBe(
      "Built-in role's instructions updated: Designer",
    );
  });
});

describe("describeEvent (Phase 11 servers)", () => {
  it("says who connected where, what ran, how it ended, and what the owner did", () => {
    const ops = { worker: "Operations Engineer", server: "Shop" };
    expect(
      describeEvent(
        event("ssh.connected", {
          ...ops,
          environment: "production",
          address: "shop@203.0.113.10:22",
        }),
      ),
    ).toBe("Operations Engineer connected to Shop (PRODUCTION) as shop@203.0.113.10:22");
    expect(
      describeEvent(
        event("ssh.command_started", {
          ...ops,
          environment: "production",
          command: "systemctl status nginx",
          cwd: "/var/www/site",
        }),
      ),
    ).toBe(
      "Operations Engineer ran on Shop (PRODUCTION): systemctl status nginx (in /var/www/site)",
    );
    const output = event("ssh.output", { server: "Shop", stream: "out", lines: ["a", "b"] });
    expect(describeEvent(output)).toBe("Output from Shop (2 lines)");
    expect(eventOutput(output)).toEqual({ lines: ["a", "b"], error: false });
    expect(eventOutput(event("ssh.connected", {}))).toBeNull();
    expect(
      describeEvent(
        event("ssh.command_finished", {
          server: "Shop",
          ending: "exited",
          exitCode: 0,
          seconds: 1.2,
        }),
      ),
    ).toBe("The command on Shop finished after 1.2 s");
    expect(
      describeEvent(event("ssh.command_finished", { server: "Shop", ending: "connectionLost" })),
    ).toBe("The command on Shop lost its connection while it ran (whether it finished is unknown)");
    expect(
      describeEvent(
        event("ssh.command_finished", {
          server: "Shop",
          ending: "stopped",
          why: "you pressed Stop all",
        }),
      ),
    ).toBe("The command on Shop was stopped (you pressed Stop all)");
    expect(
      describeEvent(
        event("ssh.host_key_changed", {
          server: "Shop",
          seen: "SHA256:new",
          expected: "SHA256:old",
        }),
      ),
    ).toBe(
      "Blocked: Shop's server ID changed — it showed SHA256:new, not the pinned SHA256:old. Nothing was sent to sign in.",
    );
    expect(describeEvent(event("control.taken_over", { kind: "server", ...ops }))).toBe(
      "You disconnected Operations Engineer from its servers",
    );
    expect(
      describeEvent(event("guard.server_added", { name: "Shop", environment: "production" })),
    ).toBe("Server added: Shop (PRODUCTION)");
    expect(
      describeEvent(event("vault.server_sign_in_stored", { name: "Shop", stored: ["password"] })),
    ).toBe("Sign-in stored for Shop: password (the value is never shown)");
    expect(describeEvent(event("ssh.tested", { server: "Shop", ok: true }))).toBe(
      "You tested Shop: connected and signed in",
    );
  });
});

describe("shownInTrail", () => {
  it("leaves out a screenshot's own record, shown with its action instead", () => {
    expect(shownInTrail(event("artifact.recorded", { type: "screenshot", path: "/x.jpg" }))).toBe(
      false,
    );
    expect(shownInTrail(event("artifact.recorded", { type: "report", path: "/r.md" }))).toBe(true);
    expect(shownInTrail(event("capability.used", { screenshot: "a1" }))).toBe(true);
  });
});

describe("describeEvent (learning, ADR-024)", () => {
  it("says what was learned and what the owner did with it", () => {
    expect(
      describeEvent(
        event("lesson.added", {
          worker: "Web Assistant",
          text: "Use the Orders page.",
          state: "waiting",
        }),
      ),
    ).toBe("Web Assistant learned something (waiting for you): Use the Orders page.");
    expect(describeEvent(event("lesson.kept", { text: "Use the Orders page." }))).toBe(
      "You kept a lesson: Use the Orders page.",
    );
    expect(describeEvent(event("learning.role_changed", { name: "Scout", auto: true }))).toBe(
      "Scout now learns on its own",
    );
    expect(describeEvent(event("learning.switched", { enabled: false }))).toBe(
      "You switched worker learning off",
    );
  });
});

describe("describeEvent (Phase 12 terminal)", () => {
  it("says where a terminal opened and how long it stayed open, never what was typed", () => {
    expect(
      describeEvent(
        event("terminal.opened", { place: "server", title: "Shop", environment: "production" }),
      ),
    ).toBe("You opened a terminal on Shop (PRODUCTION)");
    expect(
      describeEvent(
        event("terminal.opened", {
          place: "thisPc",
          title: "This PC",
          shell: "Windows PowerShell",
        }),
      ),
    ).toBe("You opened a terminal on this PC (Windows PowerShell)");
    expect(
      describeEvent(
        event("terminal.closed", {
          place: "server",
          title: "Dev box",
          seconds: 312.4,
          why: "you closed it",
        }),
      ),
    ).toBe("The terminal on Dev box closed after 5 minutes: you closed it");
    expect(
      describeEvent(event("ssh.command_stop_requested", { worker: "Operations Engineer" })),
    ).toBe("You pressed Stop on Operations Engineer's command");
  });

  it("says how long in plain words", () => {
    expect(lasted(1)).toBe("1 second");
    expect(lasted(59.4)).toBe("59 seconds");
    expect(lasted(60)).toBe("1 minute");
    expect(lasted(3900)).toBe("1 hour 5 minutes");
    expect(lasted(7200)).toBe("2 hours");
  });

  it("says who was lent, came home, and what you changed on your tile (Phase 18)", () => {
    expect(
      describeEvent(
        event("org.agent_lent", {
          title: "Security Auditor",
          to: "Campaign Supervisor",
          project: "Campaign",
          until: "objective",
        }),
      ),
    ).toBe("Security Auditor lent to Campaign Supervisor's team (Campaign) for one objective");
    expect(
      describeEvent(
        event("org.agent_returned", {
          title: "Security Auditor",
          to: "Campaign Supervisor",
          reason: "its objective is done",
        }),
      ),
    ).toBe("Security Auditor is home from Campaign Supervisor's team (its objective is done)");
    expect(describeEvent(event("org.agent_going_home", { title: "Security Auditor" }))).toBe(
      "Security Auditor goes home after the task it is on",
    );
    expect(describeEvent(event("owner.profile_changed", { changed: ["status", "picture"] }))).toBe(
      "You changed your status, picture",
    );
  });
});

describe("describeEvent (Phase 19 AI tools)", () => {
  it("says who opened an AI tool's sign-in tab and how long it ran, never what was typed", () => {
    const signIn = {
      place: "aiTool",
      runtimeId: "codex",
      action: "signIn",
      title: "Sign in · Codex",
    };
    expect(describeEvent(event("terminal.opened", signIn))).toBe("You opened Codex's sign-in");
    expect(describeEvent(event("terminal.closed", { ...signIn, seconds: 40.2, exitCode: 0 }))).toBe(
      "Codex's sign-in closed after 40 seconds",
    );
    expect(
      describeEvent(
        event("terminal.opened", { place: "aiTool", runtimeId: "claude-code", action: "signOut" }),
      ),
    ).toBe("You opened Claude Code's sign-out");
    expect(
      describeEvent(
        event("terminal.closed", {
          place: "aiTool",
          runtimeId: "claude-code",
          action: "signOut",
          seconds: 3,
        }),
      ),
    ).toBe("Claude Code's sign-out closed after 3 seconds");
  });

  it("says what changed about a sign-in, an update, and the models", () => {
    expect(
      describeEvent(
        event("ai_tool.sign_in_changed", {
          runtime: "codex",
          from: "signedOut",
          to: "subscription",
          method: "ChatGPT sign-in",
        }),
      ),
    ).toBe("Codex: signed in (ChatGPT sign-in)");
    expect(
      describeEvent(
        event("ai_tool.sign_in_changed", {
          runtime: "codex",
          from: "subscription",
          to: "signedOut",
          method: null,
        }),
      ),
    ).toBe("Codex: signed out");
    expect(
      describeEvent(
        event("ai_tool.update_available", {
          runtime: "grok",
          installed: "1.0.41",
          newest: "1.0.43",
        }),
      ),
    ).toBe("A new version of Grok is ready (1.0.43)");
    expect(
      describeEvent(
        event("ai_tool.update_started", { runtime: "grok", from: "1.0.41", by: "owner" }),
      ),
    ).toBe("Updating Grok (from 1.0.41)");
    expect(
      describeEvent(
        event("ai_tool.update_started", { runtime: "grok", from: "1.0.41", by: "automatic" }),
      ),
    ).toBe("Updating Grok (from 1.0.41), by itself");
    expect(
      describeEvent(event("ai_tool.updated", { runtime: "grok", from: "1.0.41", to: "1.0.43" })),
    ).toBe("Grok was updated to 1.0.43");
    expect(
      describeEvent(
        event("ai_tool.update_failed", {
          runtime: "grok",
          from: "1.0.41",
          reason: "the download stopped",
          oldStillWorks: true,
        }),
      ),
    ).toBe("Grok's update didn't finish: the download stopped (the old version still works)");
    expect(
      describeEvent(
        event("ai_tool.update_failed", {
          runtime: "codex",
          from: "0.50.0",
          reason: "Codex does not answer",
          oldStillWorks: false,
        }),
      ),
    ).toBe("Codex's update didn't finish: Codex does not answer");
    expect(
      describeEvent(
        event("ai_tool.update_by_hand", {
          runtime: "codex",
          newest: "0.158.0",
          message: "Codex installed with npm updates with npm.",
          automatic: true,
        }),
      ),
    ).toBe("Codex can't update itself here: Codex installed with npm updates with npm.");
    expect(describeEvent(event("ai_tool.put_back", { runtime: "grok", version: "1.0.41" }))).toBe(
      "Plenipo put back Grok 1.0.41",
    );
    expect(
      describeEvent(
        event("ai_tool.models_changed", { runtime: "grok", added: ["grok-5"], removed: [] }),
      ),
    ).toBe("Grok lists new models: grok-5");
    expect(
      describeEvent(
        event("ai_tool.models_changed", { runtime: "grok", added: [], removed: ["grok-3"] }),
      ),
    ).toBe("Grok no longer lists: grok-3");
    expect(describeEvent(event("ai_tools.auto_update_switched", { on: true }))).toBe(
      "You turned on Update AI tools by themselves",
    );
    expect(describeEvent(event("ai_tools.auto_update_switched", { on: false }))).toBe(
      "You turned off Update AI tools by themselves",
    );
    expect(
      describeEvent(
        event("guard.ai_tool_refused", {
          runtime: "kimi",
          action: "signOut",
          reason: "Kimi has no sign-out command of its own.",
        }),
      ),
    ).toBe("Blocked: Kimi has no sign-out command of its own.");
  });
});
