import type { AgentEvent } from "@plenipo/types";
import { describe, expect, it } from "vitest";

import { activity, runtime, session, turn } from "../test/agentFixtures";
import {
  activityItems,
  agentReducer,
  initialAgentState,
  isRunning,
  isWaiting,
  liaisonInfo,
  STEP_SEQ,
  stepOf,
} from "./store";

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

  it("a late command snapshot does not undo a turn that already finished", () => {
    const done = turn("t1", {
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
    });
    // Live: started, bound, finished.
    let state = agentReducer(initialAgentState, {
      type: "update",
      update: { kind: "session", ...session("s1", { activeTaskId: "t1" }) },
    });
    state = agentReducer(state, {
      type: "update",
      update: { kind: "turn", ...done },
    });
    expect(isRunning(state.sessions.s1)).toBe(false);
    state = agentReducer(state, {
      type: "update",
      update: {
        kind: "session",
        ...session("s1", { providerSessionId: "p", providerSessionConfirmed: true }),
      },
    });
    // Then the `start` response, captured while the turn was still running.
    state = agentReducer(state, {
      type: "sessionLoaded",
      detail: {
        session: session("s1", { activeTaskId: "t1", turnCount: 1 }),
        turns: [turn("t1")],
        activity: [],
      },
    });
    expect(isRunning(state.sessions.s1)).toBe(false);
    expect(state.sessions.s1?.providerSessionConfirmed).toBe(true);
    expect(state.sessions.s1?.providerSessionId).toBe("p");
    expect(state.turns.s1?.[0]?.running).toBe(false);
  });

  it("marks a session loaded only when its full history arrives", () => {
    let state = agentReducer(initialAgentState, {
      type: "update",
      update: { kind: "turn", ...turn("t3", { number: 3 }) },
    });
    expect(state.loaded.s1).toBeUndefined();
    state = agentReducer(state, {
      type: "sessionLoaded",
      detail: {
        session: session("s1"),
        turns: [turn("t1"), turn("t2", { number: 2 }), turn("t3", { number: 3 })],
        activity: [],
      },
    });
    expect(state.loaded.s1).toBe(true);
    expect(state.turns.s1?.map((t) => t.number)).toEqual([1, 2, 3]);
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

describe("agent store — waiting turns and steps (Phase 4)", () => {
  const done = (text: string) => ({
    outcome: "completed" as const,
    summary: text,
    text,
    error: null,
    providerSessionId: "p",
    model: null,
    usage: null,
    durationMs: 1,
    ignoredLines: 0,
  });
  const step1 = {
    number: 1,
    executionId: "e1",
    running: false,
    result: done("asked"),
    startedAt: 1,
    endedAt: 2,
  };
  const step2 = { ...step1, number: 2, executionId: "e2", running: true, result: null };

  function loaded() {
    return agentReducer(initialAgentState, {
      type: "overviewLoaded",
      overview: { runtimes: [], sessions: [session("s1", { activeTaskId: "t1" })], notices: [] },
    });
  }

  it("numbers activity by step", () => {
    expect([1, STEP_SEQ, STEP_SEQ + 1, 2 * STEP_SEQ + 5].map(stepOf)).toEqual([1, 1, 2, 3]);
  });

  it("tracks a turn that waits, continues, and finishes", () => {
    let state = loaded();
    state = agentReducer(state, {
      type: "update",
      update: { kind: "turn", ...turn("t1", { running: false, waiting: true, steps: [step1] }) },
    });
    expect(isRunning(state.sessions.s1)).toBe(false);
    expect(isWaiting(state.sessions.s1)).toBe(true);

    state = agentReducer(state, {
      type: "update",
      update: { kind: "turn", ...turn("t1", { running: true, steps: [step1, step2] }) },
    });
    expect(isRunning(state.sessions.s1)).toBe(true);
    expect(isWaiting(state.sessions.s1)).toBe(false);

    state = agentReducer(state, {
      type: "update",
      update: {
        kind: "turn",
        ...turn("t1", {
          running: false,
          result: done("final"),
          steps: [step1, { ...step2, running: false, result: done("final") }],
        }),
      },
    });
    expect(isRunning(state.sessions.s1) || isWaiting(state.sessions.s1)).toBe(false);
  });

  it("never lets an older snapshot move a turn backwards", () => {
    let state = loaded();
    // Live: the turn already continued with step 2.
    state = agentReducer(state, {
      type: "update",
      update: { kind: "turn", ...turn("t1", { running: true, steps: [step1, step2] }) },
    });
    // A snapshot taken while it was still waiting arrives late.
    state = agentReducer(state, {
      type: "sessionLoaded",
      detail: {
        session: session("s1", { waitingTaskId: "t1" }),
        turns: [turn("t1", { running: false, waiting: true, steps: [step1] })],
        activity: [],
      },
    });
    const t1 = state.turns.s1?.[0];
    expect(t1?.steps).toHaveLength(2);
    expect(t1?.running).toBe(true);
    expect(isWaiting(state.sessions.s1)).toBe(false);
    expect(isRunning(state.sessions.s1)).toBe(true);
  });

  it("reads Liaison settings from session metadata", () => {
    expect(liaisonInfo(session("a")).enabled).toBe(false);
    expect(
      liaisonInfo(
        session("w", {
          metadata: {
            liaison: { enabled: true, origin: "handoff", parentSessionId: "s1", depth: 2 },
          },
        }),
      ),
    ).toEqual({
      enabled: true,
      origin: "handoff",
      parentTaskId: null,
      parentSessionId: "s1",
      depth: 2,
      positionId: null,
    });
    expect(liaisonInfo(session("x", { metadata: { liaison: "nonsense" } })).origin).toBeNull();
    // The agent of an organization position, and the position it works for.
    const member = liaisonInfo(
      session("m", {
        metadata: {
          liaison: { enabled: true, origin: "member" },
          workforce: { positionId: "p1", agentId: "a1" },
        },
      }),
    );
    expect(member.origin).toBe("member");
    expect(member.positionId).toBe("p1");
  });
});
