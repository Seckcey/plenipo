import type { AgentEvent } from "@plenipo/types";
import { describe, expect, it } from "vitest";

import { activity, runtime, session, turn } from "../test/agentFixtures";
import { activityItems, agentReducer, initialAgentState, isRunning } from "./store";

const delta = (text: string): AgentEvent => ({ type: "textDelta", text });

describe("agent store", () => {
  it("loads the overview", () => {
    const state = agentReducer(initialAgentState, {
      type: "overviewLoaded",
      overview: {
        runtimes: [runtime("claude-code"), runtime("codex", false)],
        sessions: [session("b"), session("a")],
        notices: ["1 agent turn(s) from a previous session were marked interrupted."],
      },
    });
    expect(state.status).toBe("ready");
    expect(state.order).toEqual(["b", "a"]);
    expect(state.runtimes.map((r) => r.ready)).toEqual([true, false]);
    expect(state.notices).toHaveLength(1);
  });

  it("applies live updates: sessions move to the front, turns stay ordered, activity dedupes", () => {
    let state = agentReducer(initialAgentState, {
      type: "overviewLoaded",
      overview: { runtimes: [], sessions: [session("a"), session("s1")], notices: [] },
    });
    state = agentReducer(state, {
      type: "update",
      update: { kind: "session", ...session("s1", { activeTaskId: "t2" }) },
    });
    expect(state.order).toEqual(["s1", "a"]);
    expect(isRunning(state.sessions.s1)).toBe(true);
    expect("kind" in (state.sessions.s1 as object)).toBe(false);

    state = agentReducer(state, {
      type: "update",
      update: { kind: "turn", ...turn("t2", { number: 2 }) },
    });
    state = agentReducer(state, { type: "update", update: { kind: "turn", ...turn("t1") } });
    expect(state.turns.s1?.map((t) => t.number)).toEqual([1, 2]);

    for (const seq of [1, 2, 2, 1, 3]) {
      state = agentReducer(state, {
        type: "update",
        update: { kind: "activity", ...activity("t2", seq, delta(`x${seq}`)) },
      });
    }
    expect(state.activity.t2?.map((a) => a.seq)).toEqual([1, 2, 3]);
  });

  it("treats a session snapshot as authoritative up to its newest seq", () => {
    let state = initialAgentState;
    for (const seq of [1, 2, 3, 4]) {
      state = agentReducer(state, {
        type: "update",
        update: { kind: "activity", ...activity("t1", seq, delta(`${seq}`)) },
      });
    }
    // The backend coalesced deltas 1–3 into one item with seq 3.
    state = agentReducer(state, {
      type: "sessionLoaded",
      detail: {
        session: session("s1", { activeTaskId: "t1" }),
        turns: [turn("t1")],
        activity: [activity("t1", 3, delta("123"))],
      },
    });
    const texts = state.activity.t1?.map((a) => (a.event.type === "textDelta" ? a.event.text : ""));
    expect(texts).toEqual(["123", "4"]);
  });

  it("keeps a finished turn that arrived while a snapshot was in flight", () => {
    let state = agentReducer(initialAgentState, {
      type: "update",
      update: {
        kind: "turn",
        ...turn("t1", {
          running: false,
          result: {
            outcome: "completed",
            summary: "Hi",
            text: "Hi",
            error: null,
            providerSessionId: "p",
            model: null,
            usage: null,
            durationMs: 1,
            ignoredLines: 0,
          },
        }),
      },
    });
    state = agentReducer(state, {
      type: "sessionLoaded",
      detail: { session: session("s1"), turns: [turn("t1")], activity: [] },
    });
    expect(state.turns.s1?.[0]?.running).toBe(false);
  });

  it("joins streamed text and replaces it with the complete message", () => {
    const items = activityItems([
      activity("t", 1, { type: "sessionStarted", providerSessionId: "p", model: "m" }),
      activity("t", 2, delta("Hel")),
      activity("t", 3, delta("lo")),
      activity("t", 4, { type: "message", text: "Hello" }),
      activity("t", 5, { type: "toolUse", tool: "Read", summary: "/x" }),
      activity("t", 6, delta("More")),
      activity("t", 7, {
        type: "usage",
        usage: { inputTokens: 1, cachedInputTokens: 0, outputTokens: 1 },
      }),
    ]);
    expect(
      items.map((i) => (i.kind === "streaming" ? `~${i.text}` : i.activity.event.type)),
    ).toEqual(["sessionStarted", "message", "toolUse", "~More"]);
  });
});
