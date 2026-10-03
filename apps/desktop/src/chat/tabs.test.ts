import { describe, expect, it } from "vitest";

import { initialAgentState, type AgentState } from "../agents/store";
import { session } from "../test/agentFixtures";
import {
  closeTab,
  isChatTabs,
  keyOf,
  MAX_TABS,
  NO_TABS,
  openTab,
  positionSession,
  setSession,
  showTab,
  shownTabs,
} from "./tabs";

describe("the open chats (ADR-200)", () => {
  it("opens a chat in front, and opening it again shows the same one", () => {
    let t = openTab(NO_TABS, { positionId: "p1", title: "Development Manager" });
    t = openTab(t, { positionId: "p2", title: "Supervisor" });
    expect(t.tabs.map((x) => x.key)).toEqual(["position:p1", "position:p2"]);
    expect(t.active).toBe("position:p2");
    t = openTab(t, { positionId: "p1", sessionId: "s1", title: "Development Manager" });
    expect(t.tabs).toHaveLength(2);
    expect(t.active).toBe("position:p1");
    expect(t.tabs[0]?.sessionId).toBe("s1");
  });

  it("opens a position's conversation in that position's chat", () => {
    let t = openTab(NO_TABS, { positionId: "p1", sessionId: "s1", title: "Manager" });
    t = openTab(t, { sessionId: "s1", title: "Its conversation" });
    expect(t.tabs).toHaveLength(1);
    // A worker's conversation gets its own chat.
    t = openTab(t, { sessionId: "s9", title: "Developer (on call)" });
    expect(t.tabs.map((x) => x.key)).toEqual(["position:p1", "session:s9"]);
    expect(keyOf({})).toBeNull();
    expect(openTab(t, { title: "nobody" })).toBe(t);
  });

  it("closes a chat and brings the one beside it to the front", () => {
    let t = NO_TABS;
    for (const id of ["a", "b", "c"]) t = openTab(t, { positionId: id, title: id });
    t = showTab(t, "position:b");
    t = closeTab(t, "position:b");
    expect(t.active).toBe("position:c");
    t = closeTab(t, "position:c");
    expect(t.active).toBe("position:a");
    t = closeTab(t, "position:a");
    expect(t).toEqual(NO_TABS);
    expect(closeTab(t, "position:zz")).toBe(t);
  });

  it("keeps at most twelve, closing the oldest that is not in front", () => {
    let t = NO_TABS;
    for (let i = 0; i < MAX_TABS + 2; i += 1) t = openTab(t, { positionId: `p${i}`, title: "x" });
    expect(t.tabs).toHaveLength(MAX_TABS);
    expect(t.tabs[0]?.key).toBe("position:p2");
    expect(t.active).toBe(`position:p${MAX_TABS + 1}`);
  });

  it("shows up to four side by side, the front one first", () => {
    let t = NO_TABS;
    for (const id of ["a", "b", "c", "d", "e"]) t = openTab(t, { positionId: id, title: id });
    expect(shownTabs(t).map((x) => x.key)).toEqual(["position:e"]);
    t = { ...showTab(t, "position:c"), sideBySide: true };
    expect(shownTabs(t).map((x) => x.key)).toEqual([
      "position:c",
      "position:a",
      "position:b",
      "position:d",
    ]);
    expect(shownTabs(NO_TABS)).toEqual([]);
  });

  it("remembers the conversation a chat shows", () => {
    const t = openTab(NO_TABS, { positionId: "p1", title: "Manager" });
    const next = setSession(t, "position:p1", "s7");
    expect(next.tabs[0]?.sessionId).toBe("s7");
    expect(setSession(next, "position:p1", "s7")).toBe(next);
    expect(setSession(next, "position:zz", "s7")).toBe(next);
  });

  it("reads back only whole chats", () => {
    const t = openTab(NO_TABS, { positionId: "p1", title: "Manager" });
    expect(isChatTabs(JSON.parse(JSON.stringify(t)))).toBe(true);
    expect(isChatTabs(null)).toBe(false);
    expect(isChatTabs({ ...t, sideBySide: "yes" })).toBe(false);
    expect(
      isChatTabs({ ...t, tabs: [{ key: "x", positionId: null, sessionId: null, title: "x" }] }),
    ).toBe(false);
  });

  it("finds a position's agent's newest open conversation", () => {
    const member = (id: string, positionId: string, state: "open" | "closed" = "open") =>
      session(id, {
        state,
        metadata: { liaison: { origin: "member" }, workforce: { positionId } },
      });
    const state: AgentState = {
      ...initialAgentState,
      sessions: {
        s1: member("s1", "p1", "closed"),
        s2: member("s2", "p1"),
        s3: member("s3", "p2"),
      },
      order: ["s1", "s2", "s3"],
    };
    expect(positionSession(state, "p1")).toBe("s2");
    expect(positionSession(state, "p2")).toBe("s3");
    expect(positionSession(state, "p9")).toBeNull();
  });
});
