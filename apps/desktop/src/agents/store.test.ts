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
  saidAt,
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
    // A piece seen twice is skipped; the streamed words are kept as one item, at the newest seq.
    expect(state.activity.t2?.map((a) => a.seq)).toEqual([3]);
    expect(state.activity.t2?.map((a) => (a.event as { text: string }).text)).toEqual(["x1x2x3"]);
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
    expect(items.map((i) => (i.kind === "event" ? i.activity.event.type : `~${i.text}`))).toEqual([
      "sessionStarted",
      "message",
      "toolUse",
      "~More",
    ]);
  });
});

describe("agent store — streamed thinking and words, joined", () => {
  const think = (text: string): AgentEvent => ({ type: "reasoning", text });
  const tool: AgentEvent = { type: "toolUse", tool: "Read", summary: "/x" };
  const textOf = (a: { event: AgentEvent }) =>
    a.event.type === "textDelta" || a.event.type === "reasoning" ? a.event.text : a.event.type;
  const live = (state: typeof initialAgentState, taskId: string, seq: number, event: AgentEvent) =>
    agentReducer(state, {
      type: "update",
      update: { kind: "activity", ...activity(taskId, seq, event) },
    });
  const row = (i: ReturnType<typeof activityItems>[number]) =>
    i.kind === "event" ? i.activity.event.type : `${i.kind}:${i.text}`;

  it("shows Claude's thinking as one row that grows, not a row for each piece", () => {
    const items = activityItems([
      activity("t", 1, { type: "status", phase: "thinking", text: "Thinking" }),
      activity("t", 2, think("I")),
      activity("t", 3, think(" sh")),
      activity("t", 4, think("ould check")),
      activity("t", 5, delta("Here")),
      activity("t", 6, delta(" it is.")),
      activity("t", 7, { type: "message", text: "Here it is." }),
      activity("t", 8, tool),
      activity("t", 9, think("Now")),
      activity("t", 10, think(" the tests.")),
    ]);
    // Its sign that it began to think is said by the thinking row itself.
    expect(items.map(row)).toEqual([
      "thinking:I should check",
      "message",
      "toolUse",
      "thinking:Now the tests.",
    ]);
  });

  it("keeps the sign that it began to think only until its thinking's words come", () => {
    const began = activity("t", 1, { type: "status", phase: "thinking", text: "Thinking" });
    // Before the words come (or when the AI tool does not show its thinking), the sign shows.
    expect(activityItems([began]).map(row)).toEqual(["status"]);
    expect(activityItems([began, activity("t", 2, delta("Done."))]).map(row)).toEqual([
      "status",
      "streaming:Done.",
    ]);
    // Once they come, the thinking row says it, once.
    expect(activityItems([began, activity("t", 2, think("Hm"))]).map(row)).toEqual(["thinking:Hm"]);
    // Other statuses stay, words or not (waiting for the AI company, say).
    const waiting = activity("t", 1, { type: "status", phase: "waiting", text: "Waiting" });
    expect(activityItems([waiting, activity("t", 2, think("Hm"))]).map(row)).toEqual([
      "status",
      "thinking:Hm",
    ]);
  });

  it("keeps streamed pieces as one item each for words and for thinking, and a step apart", () => {
    let state = initialAgentState;
    const events = [think("a"), think("b"), delta("c"), delta("d"), tool, delta("e")];
    events.forEach((e, i) => {
      state = live(state, "t1", i + 1, e);
    });
    const kept = state.activity.t1 ?? [];
    expect(kept.map(textOf)).toEqual(["ab", "cd", "toolUse", "e"]);
    // Each joined item takes its newest piece's number and keeps its first piece's time (when
    // the thinking began, as Core's buffer keeps it); when it last said something is kept too.
    expect(kept.map((a) => [a.seq, a.ts, saidAt(a)])).toEqual([
      [2, 1, 2],
      [4, 3, 4],
      [5, 5, 5],
      [6, 6, 6],
    ]);
    // A piece that does not come right after (another step) starts an item of its own.
    state = live(state, "t1", STEP_SEQ + 1, delta("f"));
    expect((state.activity.t1 ?? []).map(textOf)).toEqual(["ab", "cd", "toolUse", "e", "f"]);
  });

  it("never joins across steps, even where one step's numbers end and the next's begin", () => {
    let state = live(initialAgentState, "t1", STEP_SEQ - 1, think("a"));
    state = live(state, "t1", STEP_SEQ, think("b"));
    state = live(state, "t1", STEP_SEQ + 1, think("c"));
    expect((state.activity.t1 ?? []).map((a) => [stepOf(a.seq), textOf(a)])).toEqual([
      [1, "ab"],
      [2, "c"],
    ]);
  });

  it("never lets a long thought push the turn's earlier steps out", () => {
    let state = live(initialAgentState, "t1", 1, tool);
    for (let seq = 2; seq <= 3001; seq += 1) state = live(state, "t1", seq, think("x"));
    const kept = state.activity.t1 ?? [];
    expect(kept[0]?.event.type).toBe("toolUse");
    expect(kept.length).toBeLessThan(10);
    expect(kept.slice(1).map(textOf).join("")).toBe("x".repeat(3000));
  });

  it("names a joined row by its first piece, so the row stays while it grows", () => {
    let state = live(initialAgentState, "t1", 1, think("I"));
    state = live(state, "t1", 2, think(" think"));
    const before = activityItems(state.activity.t1 ?? []);
    state = live(state, "t1", 3, think(" so"));
    const after = activityItems(state.activity.t1 ?? []);
    expect(after.map(row)).toEqual(["thinking:I think so"]);
    expect(after[0]?.key).toBe(before[0]?.key);
    // Words and thinking that start at the same place are named apart.
    expect(activityItems([activity("t", 1, delta("a"))])[0]?.key).not.toBe(
      activityItems([activity("t", 1, think("a"))])[0]?.key,
    );
  });

  it("takes from a snapshot only the pieces it does not have yet", () => {
    // Live pieces 3–5 came (the window started listening late); the snapshot has 1–4 as one.
    let state = initialAgentState;
    for (const [seq, text] of [
      [3, "c"],
      [4, "d"],
      [5, "e"],
    ] as const) {
      state = live(state, "t1", seq, think(text));
    }
    state = agentReducer(state, {
      type: "sessionLoaded",
      detail: {
        session: session("s1", { activeTaskId: "t1" }),
        turns: [turn("t1")],
        activity: [activity("t1", 4, think("abcd"))],
      },
    });
    expect((state.activity.t1 ?? []).map(textOf)).toEqual(["abcd", "e"]);
    expect(activityItems(state.activity.t1 ?? []).map(row)).toEqual(["thinking:abcde"]);
    // The next piece goes on after it, never twice.
    state = live(state, "t1", 6, think("f"));
    expect(activityItems(state.activity.t1 ?? []).map(row)).toEqual(["thinking:abcdef"]);
    // A snapshot that has all of it keeps nothing of the live item.
    state = agentReducer(state, {
      type: "sessionLoaded",
      detail: {
        session: session("s1", { activeTaskId: "t1" }),
        turns: [turn("t1")],
        activity: [activity("t1", 6, think("abcdef"))],
      },
    });
    expect((state.activity.t1 ?? []).map(textOf)).toEqual(["abcdef"]);
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

  it("keeps a recorded wait over a snapshot read while the step was being recorded", () => {
    let state = loaded();
    state = agentReducer(state, {
      type: "update",
      update: { kind: "turn", ...turn("t1", { running: false, waiting: true, steps: [step1] }) },
    });
    // Read in the instant between recording the wait and releasing the step: "running", with
    // no step left to run.
    state = agentReducer(state, {
      type: "sessionLoaded",
      detail: {
        session: session("s1", { activeTaskId: "t1" }),
        turns: [turn("t1", { running: true, steps: [step1] })],
        activity: [],
      },
    });
    expect(state.turns.s1?.[0]?.waiting).toBe(true);
    expect(isWaiting(state.sessions.s1)).toBe(true);
    expect(isRunning(state.sessions.s1)).toBe(false);

    // The same for a turn that finished: the snapshot never brings "running" back.
    state = agentReducer(state, {
      type: "update",
      update: { kind: "turn", ...turn("t1", { running: false, result: done("final") }) },
    });
    state = agentReducer(state, {
      type: "sessionLoaded",
      detail: {
        session: session("s1", { activeTaskId: "t1" }),
        turns: [turn("t1", { running: true, result: done("final") })],
        activity: [],
      },
    });
    expect(state.turns.s1?.[0]?.running).toBe(false);
    expect(isRunning(state.sessions.s1)).toBe(false);
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
