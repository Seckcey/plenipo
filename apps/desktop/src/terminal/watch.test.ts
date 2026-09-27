import type { LedgerEvent } from "@plenipo/types";
import { describe, expect, it } from "vitest";

import {
  applyWatchEvent,
  applyWatchEvents,
  MAX_COMMAND_LINES,
  runningCommand,
  watchLines,
  watchTitle,
  type WatchCommand,
} from "./watch";
import { pieces } from "./words";

let seq = 0;
function ev(eventType: string, payload: Record<string, unknown>, taskId = "task-1"): LedgerEvent {
  seq += 1;
  return {
    seq,
    id: `e${seq}`,
    taskId,
    executionId: null,
    source: "guard",
    destination: null,
    eventType,
    payload,
    createdAt: 1_700_000_000_000 + seq,
  };
}

const connected = ev("ssh.connected", {
  grantId: "g1",
  worker: "Operations Engineer",
  serverId: "s1",
  server: "Shop",
  environment: "production",
  address: "shop@web01:22",
});

describe("watch tabs", () => {
  it("opens a tab when a worker connects, named for the worker and the server", () => {
    const tabs = applyWatchEvent([], connected);
    expect(tabs).toHaveLength(1);
    const tab = tabs[0]!;
    expect(watchTitle(tab)).toBe("Operations Engineer · Shop");
    expect(tab.environment).toBe("production");
    expect(tab.connected).toBe(true);
    // The same connection again changes nothing but the state.
    expect(applyWatchEvent(tabs, connected)).toHaveLength(1);
  });

  it("shows each command and its output as it arrives, then how it ended", () => {
    const tabs = applyWatchEvents(
      [],
      [
        connected,
        ev("ssh.command_started", {
          grantId: "g1",
          serverId: "s1",
          commandId: "c1",
          command: "systemctl status nginx",
          cwd: "/srv",
        }),
        ev("ssh.output", { commandId: "c1", stream: "out", lines: ["● nginx", "active"] }),
        ev("ssh.output", { commandId: "c1", stream: "err", lines: ["warning"] }),
      ],
    );
    const tab = tabs[0]!;
    const running = runningCommand(tab);
    expect(running?.command).toBe("systemctl status nginx");
    expect(running?.lines).toEqual([
      { stream: "out", text: "● nginx" },
      { stream: "out", text: "active" },
      { stream: "err", text: "warning" },
    ]);
    const done = applyWatchEvent(
      tabs,
      ev("ssh.command_finished", { commandId: "c1", ending: "exited", exitCode: 0, seconds: 0.4 }),
    );
    expect(runningCommand(done[0]!)).toBeNull();
    const lines = watchLines(done[0]!);
    expect(lines.map((l) => [l.text, l.tone])).toEqual([
      ["$ systemctl status nginx   (in /srv)", "command"],
      ["● nginx", "normal"],
      ["active", "normal"],
      ["warning", "error"],
      ["Finished after 0.4 s", "ok"],
    ]);
  });

  it("marks a command stopping when the owner presses Stop, and stopped when it ends", () => {
    let tabs = applyWatchEvents(
      [],
      [
        connected,
        ev("ssh.command_started", {
          grantId: "g1",
          serverId: "s1",
          commandId: "c2",
          command: "sleep 60",
        }),
        ev("ssh.command_stop_requested", { commandId: "c2", grantId: "g1" }),
      ],
    );
    expect(runningCommand(tabs[0]!)?.state).toBe("stopping");
    tabs = applyWatchEvent(
      tabs,
      ev("ssh.command_finished", { commandId: "c2", ending: "stopped", why: "you pressed Stop" }),
    );
    expect(watchLines(tabs[0]!).at(-1)?.text).toBe("Stopped (you pressed Stop)");
  });

  it("shows refused commands as refused, in the server's tab", () => {
    const tabs = applyWatchEvents(
      [],
      [
        connected,
        ev("ssh.connected", {
          grantId: "g1",
          worker: "Operations Engineer",
          serverId: "s2",
          server: "Dev box",
        }),
        ev("guard.denied", {
          grantId: "g1",
          tool: "ssh_run",
          summary: "run rm -rf /srv on Shop",
          reason: "Blocked: never run on production",
          server: "Shop",
        }),
        // Other tools' refusals, and other workers', are not shown here.
        ev("guard.denied", { grantId: "g1", tool: "write_file", summary: "write x" }),
        ev("guard.denied", { grantId: "g9", tool: "ssh_run", summary: "run ls on Shop" }),
      ],
    );
    const shop = tabs.find((t) => t.server === "Shop")!;
    const dev = tabs.find((t) => t.server === "Dev box")!;
    expect(shop.items).toHaveLength(1);
    expect(watchLines(shop)[0]).toMatchObject({
      text: "Refused: run rm -rf /srv on Shop — Blocked: never run on production",
      tone: "warn",
    });
    expect(dev.items).toHaveLength(0);
  });

  it("stays readable after the worker disconnects", () => {
    const tabs = applyWatchEvents(
      [],
      [
        connected,
        ev("ssh.command_started", {
          grantId: "g1",
          serverId: "s1",
          commandId: "c3",
          command: "uptime",
        }),
        ev("ssh.disconnected", {
          grantId: "g1",
          serverId: "s1",
          why: "you disconnected the worker",
        }),
      ],
    );
    const tab = tabs[0]!;
    expect(tab.connected).toBe(false);
    expect(watchLines(tab).at(-1)?.text).toBe("Disconnected: you disconnected the worker");
    expect(watchLines(tab)[0]?.text).toBe("$ uptime");
  });

  it("keeps a long command's newest output and counts the rest", () => {
    let tabs = applyWatchEvents(
      [],
      [
        connected,
        ev("ssh.command_started", {
          grantId: "g1",
          serverId: "s1",
          commandId: "c4",
          command: "yes",
        }),
      ],
    );
    const many = Array.from({ length: MAX_COMMAND_LINES + 5 }, (_, i) => `line ${i}`);
    tabs = applyWatchEvent(tabs, ev("ssh.output", { commandId: "c4", stream: "out", lines: many }));
    const c = tabs[0]!.items[0] as WatchCommand;
    expect(c.lines).toHaveLength(MAX_COMMAND_LINES);
    expect(c.dropped).toBe(5);
    expect(c.lines[0]?.text).toBe("line 5");
    expect(watchLines(tabs[0]!)[1]?.text).toBe("… 5 earlier lines are in the Activity trail");
  });

  it("ignores events that are not about a worker's server work", () => {
    const tabs = applyWatchEvent([], connected);
    expect(applyWatchEvent(tabs, ev("task.state_changed", { from: "queued", to: "running" }))).toBe(
      tabs,
    );
    expect(applyWatchEvent(tabs, ev("ssh.output", { commandId: "nope", lines: ["x"] }))).toBe(tabs);
  });
});

describe("a command with more output than Plenipo keeps", () => {
  it("says so while it runs, and says how many lines were not kept when it ends", () => {
    let tabs = applyWatchEvents(
      [],
      [
        ev("ssh.connected", {
          grantId: "g1",
          worker: "Operations Engineer",
          serverId: "s1",
          server: "Shop",
          environment: "test",
        }),
        ev("ssh.command_started", {
          grantId: "g1",
          commandId: "c1",
          serverId: "s1",
          command: "apt-get upgrade",
        }),
        ev("ssh.output", {
          grantId: "g1",
          commandId: "c1",
          stream: "out",
          lines: Array.from({ length: 2000 }, (_, i) => `line ${i}`),
        }),
      ],
    );
    expect(watchLines(tabs[0]!).at(-1)?.text).toBe(
      "… later lines are not shown (Plenipo keeps the first 2000 lines of a command)",
    );
    tabs = applyWatchEvents(tabs, [
      ev("ssh.command_finished", {
        grantId: "g1",
        commandId: "c1",
        ending: "exited",
        exitCode: 0,
        seconds: 240,
        lines: 5000,
      }),
    ]);
    const shown = watchLines(tabs[0]!).map((l) => l.text);
    expect(shown).toContain(
      "… 3000 more lines were not kept (Plenipo keeps the first 2000 lines of a command)",
    );
    expect(shown.at(-1)).toBe("Finished after 240.0 s");
  });
});

describe("pieces (a large paste)", () => {
  it("cuts text into pieces of at most the given size, in order", () => {
    expect(pieces("abcdefg", 3)).toEqual(["abc", "def", "g"]);
    expect(pieces("", 3)).toEqual([]);
    expect(pieces("abc", 3)).toEqual(["abc"]);
  });

  it("never splits a character in two", () => {
    const text = "ab😀cd";
    expect(pieces(text, 3)).toEqual(["ab", "😀c", "d"]);
    expect(pieces(text, 3).join("")).toBe(text);
  });
});
