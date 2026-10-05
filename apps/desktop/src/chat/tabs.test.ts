import { describe, expect, it } from "vitest";

import { initialAgentState, type AgentState } from "../agents/store";
import { session } from "../test/agentFixtures";
import {
  closeTab,
  isChatTabs,
  keyOf,
  MAX_CHAT_WINDOWS,
  MAX_TABS,
  NO_TABS,
  openTab,
  popOutTab,
  positionSession,
  putBackTab,
  readChatTabs,
  setSession,
  showTab,
  shownTabs,
  slotOf,
  tabFor,
  tabInSlot,
  type ChatTabs,
} from "./tabs";

describe("the open chats (ADR-200)", () => {
  it("makes a chat for a position or a conversation, and none for nothing", () => {
    expect(tabFor({ positionId: "p1", title: "  Development Manager " })).toEqual({
      key: "position:p1",
      positionId: "p1",
      sessionId: null,
      title: "Development Manager",
    });
    expect(tabFor({ sessionId: "s9", title: "" })).toEqual({
      key: "session:s9",
      positionId: null,
      sessionId: "s9",
      title: "Agent",
    });
    expect(tabFor({ title: "Nobody" })).toBeNull();
  });

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

describe("chats in windows of their own (ADR-203)", () => {
  const open = (...ids: string[]): ChatTabs =>
    ids.reduce((t, id) => openTab(t, { positionId: id, title: id }), NO_TABS);
  const popAll = (t: ChatTabs, ids: string[]) =>
    ids.reduce((s, id) => popOutTab(s, `position:${id}`) ?? s, t);

  it("gives each chat the first free window, up to six, and keeps a window it has", () => {
    let t = open("a", "b", "c");
    t = popOutTab(t, "position:a") ?? t;
    t = popOutTab(t, "position:c") ?? t;
    expect(t.popped).toEqual([
      { key: "position:a", slot: 1 },
      { key: "position:c", slot: 2 },
    ]);
    expect(slotOf(t, "position:c")).toBe(2);
    expect(slotOf(t, "position:b")).toBeNull();
    expect(tabInSlot(t, 2)?.title).toBe("c");
    expect(tabInSlot(t, 3)).toBeNull();
    // Asked again: the same window.
    expect(popOutTab(t, "position:a")).toBe(t);
    // A chat that is not open has no window to get.
    expect(popOutTab(t, "position:zz")).toBeNull();
    // Window 1 freed: the next chat takes it.
    t = putBackTab(t, "position:a");
    t = popOutTab(t, "position:b") ?? t;
    expect(slotOf(t, "position:b")).toBe(1);
  });

  it("has six windows at most: the seventh chat stays in the panel", () => {
    const ids = ["a", "b", "c", "d", "e", "f", "g"];
    const t = popAll(open(...ids), ids.slice(0, MAX_CHAT_WINDOWS));
    expect(t.popped.map((p) => p.slot)).toEqual([1, 2, 3, 4, 5, 6]);
    expect(popOutTab(t, "position:g")).toBeNull();
  });

  it("puts a chat back in front of the panel, and closing a chat closes its window", () => {
    let t = popAll(open("a", "b"), ["a", "b"]);
    t = showTab(t, "position:b");
    t = putBackTab(t, "position:a");
    expect(t.active).toBe("position:a");
    expect(slotOf(t, "position:a")).toBeNull();
    expect(putBackTab(t, "position:a")).toBe(t);
    t = closeTab(t, "position:b");
    expect(t.popped).toEqual([]);
  });

  it("never closes a chat with a window of its own to make room for a new one", () => {
    const ids = Array.from({ length: MAX_TABS }, (_, i) => `p${i}`);
    let t = popAll(open(...ids), ["p0", "p1"]);
    t = openTab(t, { positionId: "new", title: "new" });
    expect(t.tabs).toHaveLength(MAX_TABS);
    expect(t.tabs.map((x) => x.key)).toContain("position:p0");
    expect(t.tabs.map((x) => x.key)).toContain("position:p1");
    expect(t.tabs.map((x) => x.key)).not.toContain("position:p2");
    expect(t.popped.map((p) => p.key)).toEqual(["position:p0", "position:p1"]);
  });

  it("side by side leaves out the chats in windows of their own", () => {
    let t = popAll(open("a", "b", "c"), ["b"]);
    t = { ...showTab(t, "position:a"), sideBySide: true };
    expect(shownTabs(t).map((x) => x.key)).toEqual(["position:a", "position:c"]);
    // One at a time, the front chat shows even with a window of its own (the panel says so).
    t = { ...showTab(t, "position:b"), sideBySide: false };
    expect(shownTabs(t).map((x) => x.key)).toEqual(["position:b"]);
    // Side by side with every chat in a window: the front one, which says where it is.
    t = { ...popAll(t, ["a", "c"]), sideBySide: true };
    expect(shownTabs(t).map((x) => x.key)).toEqual(["position:b"]);
  });

  it("reads back which chats have windows, and chats kept before they could", () => {
    const t = popAll(open("a", "b"), ["b"]);
    expect(readChatTabs(JSON.parse(JSON.stringify(t)))).toEqual(t);
    // Kept by an older Plenipo: no windows.
    const old: Partial<ChatTabs> = { ...t };
    delete old.popped;
    expect(readChatTabs(JSON.parse(JSON.stringify(old)))).toEqual({ ...t, popped: [] });
    for (const popped of [
      [{ key: "position:b", slot: 7 }],
      [{ key: "position:b", slot: 1.5 }],
      [{ key: "position:zz", slot: 1 }],
      [
        { key: "position:a", slot: 1 },
        { key: "position:b", slot: 1 },
      ],
      [
        { key: "position:b", slot: 1 },
        { key: "position:b", slot: 2 },
      ],
      "b",
    ]) {
      expect(isChatTabs({ ...t, popped })).toBe(false);
      expect(readChatTabs({ ...t, popped })).toEqual(NO_TABS);
    }
  });
});
