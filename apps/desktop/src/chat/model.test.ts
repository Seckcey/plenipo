import type {
  AgentActivity,
  AgentEvent,
  AgentSessionDetail,
  AgentTurn,
  TurnOutcome,
} from "@plenipo/types";
import { describe, expect, it } from "vitest";

import {
  applyActivity,
  applyTurn,
  doingNow,
  emptySession,
  endUnfinished,
  filesOf,
  fromDetail,
  isBusy,
  mergeDetail,
  stuckTurns,
  type ChatSession,
} from "./model";
import { toolPhrase } from "./words";

const SESSION = "session-1";
const TASK = "task-1";

function act(seq: number, event: AgentEvent, ts = 1000 + seq * 100, task = TASK): AgentActivity {
  return { sessionId: SESSION, taskId: task, seq, ts, event };
}

function record(over: Partial<AgentTurn> = {}): AgentTurn {
  return {
    taskId: TASK,
    sessionId: SESSION,
    number: 1,
    objective: "Write a script that clears my temp files",
    requestedBy: { kind: "owner" },
    executionId: null,
    running: true,
    waiting: false,
    result: null,
    steps: [],
    startedAt: 1000,
    endedAt: null,
    ...over,
  };
}

function finished(text: string | null, outcome: TurnOutcome = "completed"): AgentTurn {
  return record({
    running: false,
    endedAt: 9000,
    result: {
      outcome,
      summary: "done",
      text,
      error: null,
      providerSessionId: null,
      model: "sonnet",
      usage: { inputTokens: 10, cachedInputTokens: 0, outputTokens: 5 },
      durationMs: 8000,
      ignoredLines: 0,
    },
  });
}

function run(events: AgentEvent[], from: ChatSession = emptySession(SESSION)): ChatSession {
  return events.reduce((s, e, i) => applyActivity(s, act(i + 1, e)), from);
}

function turn(session: ChatSession) {
  const t = session.turns[0];
  if (!t) throw new Error("no turn");
  return t;
}

describe("words", () => {
  it("joins pieces into one part, and the whole message replaces them", () => {
    let s = run([
      { type: "textDelta", text: "I'll write " },
      { type: "textDelta", text: "the script" },
    ]);
    expect(turn(s).parts).toHaveLength(1);
    expect(turn(s).parts[0]).toMatchObject({
      kind: "text",
      text: "I'll write the script",
      streaming: true,
    });
    s = applyActivity(s, act(3, { type: "message", text: "I'll write the script now." }));
    expect(turn(s).parts).toHaveLength(1);
    expect(turn(s).parts[0]).toMatchObject({
      kind: "text",
      text: "I'll write the script now.",
      streaming: false,
    });
  });

  it("does not repeat a message it has already shown", () => {
    const s = run([
      { type: "message", text: "Done." },
      { type: "message", text: "Done." },
    ]);
    expect(turn(s).parts).toHaveLength(1);
  });

  it("keeps a long answer whole, however many pieces it comes in", () => {
    const pieces: AgentEvent[] = Array.from({ length: 3000 }, (_, i) => ({
      type: "textDelta",
      text: `w${i} `,
    }));
    const s = run(pieces);
    expect(turn(s).parts).toHaveLength(1);
    const p = turn(s).parts[0];
    expect(p?.kind === "text" && p.text.startsWith("w0 w1 w2")).toBe(true);
    expect(p?.kind === "text" && p.text.endsWith("w2999 ")).toBe(true);
  });
});

describe("thinking", () => {
  it("opens while thoughts arrive and closes at the next thing", () => {
    let s = run([
      { type: "reasoning", text: "The owner wants " },
      { type: "reasoning", text: "a script." },
    ]);
    const open = turn(s).parts[0];
    expect(open).toMatchObject({
      kind: "thinking",
      text: "The owner wants a script.",
      endedAt: null,
    });
    s = applyActivity(s, act(3, { type: "textDelta", text: "Here it is." }));
    expect(turn(s).parts[0]).toMatchObject({ kind: "thinking" });
    const closed = turn(s).parts[0];
    expect(closed?.kind === "thinking" && closed.endedAt).not.toBeNull();
  });
});

describe("tool calls", () => {
  it("groups calls that follow each other, and matches each result to its call", () => {
    const s = run([
      { type: "toolUse", tool: "mcp__plenipo__read_file", summary: "README.md" },
      { type: "toolUse", tool: "mcp__plenipo__write_file", summary: "clear-temp.ps1" },
      { type: "toolResult", tool: null, isError: false, summary: "1 line" },
      { type: "toolResult", tool: null, isError: true, summary: "refused" },
    ]);
    expect(turn(s).parts).toHaveLength(1);
    const group = turn(s).parts[0];
    expect(group?.kind).toBe("tools");
    if (group?.kind !== "tools") return;
    expect(group.calls.map((c) => [c.state, c.result])).toEqual([
      ["done", "1 line"],
      ["failed", "refused"],
    ]);
  });

  it("matches a result by its ID when it has one", () => {
    const s = run([
      { type: "toolUse", tool: "run_command", summary: "cargo test", id: "a" },
      { type: "toolUse", tool: "run_command", summary: "cargo fmt", id: "b" },
      { type: "toolResult", tool: null, isError: false, summary: "ok", id: "b" },
    ]);
    const group = turn(s).parts[0];
    if (group?.kind !== "tools") throw new Error("no group");
    expect(group.calls.map((c) => c.state)).toEqual(["running", "done"]);
  });

  it("starts a new group after words", () => {
    const s = run([
      { type: "toolUse", tool: "read_file", summary: "a.txt" },
      { type: "message", text: "Now I know." },
      { type: "toolUse", tool: "write_file", summary: "b.txt" },
    ]);
    expect(turn(s).parts.map((p) => p.kind)).toEqual(["tools", "text", "tools"]);
  });

  it("ends words that were being written when a tool starts", () => {
    const s = run([
      { type: "textDelta", text: "Let me check" },
      { type: "toolUse", tool: "read_file", summary: "a.txt" },
    ]);
    const words = turn(s).parts[0];
    expect(words).toMatchObject({ kind: "text", streaming: false });
  });
});

describe("notes and waiting", () => {
  it("shows a notice as a note", () => {
    const s = run([{ type: "notice", level: "warning", text: "Something odd" }]);
    expect(turn(s).parts[0]).toMatchObject({
      kind: "note",
      level: "warning",
      text: "Something odd",
    });
  });

  it("holds a wait until anything else happens", () => {
    let s = run([
      { type: "status", phase: "waiting", text: "Claude is busy. Trying again (3 of 10)." },
    ]);
    expect(turn(s).status).toMatchObject({ text: "Claude is busy. Trying again (3 of 10)." });
    s = applyActivity(s, act(2, { type: "textDelta", text: "Hello" }));
    expect(turn(s).status).toBeNull();
  });
});

describe("a turn's record", () => {
  it("sets what was asked and how it ended", () => {
    let s = run([{ type: "textDelta", text: "Done." }]);
    s = applyTurn(s, finished("Done."));
    const t = turn(s);
    expect(t.ask).toBe("Write a script that clears my temp files");
    expect(t.state).toBe("done");
    expect(t.answer).toBe("Done.");
    expect(t.parts).toHaveLength(1);
    expect(t.parts[0]).toMatchObject({ kind: "text", streaming: false });
    expect(t.usage).toMatchObject({ outputTokens: 5 });
    expect(isBusy(s)).toBe(false);
  });

  it("adds the answer when no words on screen say it", () => {
    let s = run([{ type: "toolUse", tool: "read_file", summary: "a.txt" }]);
    s = applyTurn(s, finished("The script is in clear-temp.ps1."));
    expect(turn(s).parts.map((p) => p.kind)).toEqual(["tools", "text"]);
  });

  it("completes the last words when they are the start of the answer", () => {
    let s = run([{ type: "textDelta", text: "The script is in" }]);
    s = applyTurn(s, finished("The script is in clear-temp.ps1."));
    expect(turn(s).parts).toHaveLength(1);
    expect(turn(s).parts[0]).toMatchObject({ text: "The script is in clear-temp.ps1." });
  });

  it("says stopped for a task you stopped, and failed with the reason for the others", () => {
    const stopped = applyTurn(emptySession(SESSION), finished(null, "cancelled"));
    expect(turn(stopped).state).toBe("stopped");
    const failed = applyTurn(emptySession(SESSION), {
      ...finished(null, "failed"),
      result: {
        ...(finished(null, "failed").result as NonNullable<AgentTurn["result"]>),
        summary: "Claude Code ended with an error",
        error: "Not signed in",
      },
    });
    expect(turn(failed).state).toBe("failed");
    expect(turn(failed).problem).toBe("Not signed in");
  });

  it("shows a turn whose end was missed as stopped, until its real end comes", () => {
    const writing = run([{ type: "textDelta", text: "Half an ans" }]);
    expect(isBusy(writing)).toBe(true);
    // Its record says it is over without saying how: not running, not waiting, no result.
    const stuck = stuckTurns([record({ running: false })]);
    expect(stuck).toEqual(new Set([TASK]));
    const ended = endUnfinished(writing, 5000, stuck);
    expect(isBusy(ended)).toBe(false);
    expect(turn(ended)).toMatchObject({
      state: "stopped",
      endedAt: 5000,
      problem: "It was no longer running.",
    });
    expect(turn(ended).parts[0]).toMatchObject({ text: "Half an ans", streaming: false });
    // A turn over already is left as it is.
    const done = applyTurn(emptySession(SESSION), finished("All done."));
    expect(endUnfinished(done, 5000, stuck)).toBe(done);
    // Its real end, when it comes, replaces it.
    expect(turn(applyTurn(ended, finished("All done."))).state).toBe("done");
  });

  it("never ends a turn whose record says it is running or waiting", () => {
    expect(
      stuckTurns([record(), record({ running: false, waiting: true }), finished("x")]),
    ).toEqual(new Set());
    const writing = run([{ type: "textDelta", text: "Still going" }]);
    expect(endUnfinished(writing, 5000, new Set())).toBe(writing);
    // Only the stuck one ends; another turn running beside it goes on.
    const two = applyTurn(writing, record({ taskId: "task-2", number: 2 }));
    const ended = endUnfinished(two, 5000, new Set([TASK]));
    expect(ended.turns.map((t) => t.state)).toEqual(["stopped", "working"]);
  });

  it("is waiting while its team answers, and working while a step runs", () => {
    const waiting = applyTurn(emptySession(SESSION), record({ running: false, waiting: true }));
    expect(turn(waiting).state).toBe("waiting");
    expect(isBusy(waiting)).toBe(true);
    const working = applyTurn(emptySession(SESSION), record());
    expect(turn(working).state).toBe("working");
  });

  it("ignores another conversation's pieces and records", () => {
    const s = emptySession(SESSION);
    expect(
      applyActivity(s, { ...act(1, { type: "message", text: "x" }), sessionId: "other" }),
    ).toBe(s);
    expect(applyTurn(s, record({ sessionId: "other" }))).toBe(s);
  });
});

describe("a repeated piece", () => {
  it("is applied once", () => {
    let s = emptySession(SESSION);
    s = applyActivity(s, act(1, { type: "textDelta", text: "Hi" }));
    s = applyActivity(s, act(2, { type: "textDelta", text: " there" }));
    const again = applyActivity(s, act(2, { type: "textDelta", text: " there" }));
    expect(again).toBe(s);
    const older = applyActivity(s, act(1, { type: "textDelta", text: "Hi" }));
    expect(older).toBe(s);
  });
});

describe("from what Plenipo keeps", () => {
  const session = (over: Partial<AgentSessionDetail["session"]> = {}) =>
    ({
      id: SESSION,
      runtime: "claude-code",
      title: "VP",
      ...over,
    }) as AgentSessionDetail["session"];

  it("builds the turns in order with their answers", () => {
    const detail: AgentSessionDetail = {
      session: session(),
      turns: [
        { ...finished("First answer."), taskId: "t1", number: 1, objective: "First" },
        { ...finished("Second answer."), taskId: "t2", number: 2, objective: "Second" },
      ],
      activity: [],
    };
    const s = fromDetail(detail);
    expect(s.turns.map((t) => [t.ask, t.answer])).toEqual([
      ["First", "First answer."],
      ["Second", "Second answer."],
    ]);
    // With no live pieces kept, the answer is the whole of the agent's side.
    expect(s.turns[0]?.parts.map((p) => p.kind)).toEqual(["text"]);
  });

  it("joins the pieces still kept to a turn that is running", () => {
    const detail: AgentSessionDetail = {
      session: session(),
      turns: [record()],
      activity: [
        act(1, { type: "toolUse", tool: "read_file", summary: "a.txt" }),
        act(2, { type: "toolResult", tool: null, isError: false, summary: "ok" }),
        act(3, { type: "textDelta", text: "Reading" }),
      ],
    };
    const s = fromDetail(detail);
    expect(turn(s).state).toBe("working");
    expect(turn(s).parts.map((p) => p.kind)).toEqual(["tools", "text"]);
    // The live feed repeats what was kept: nothing is doubled.
    const again = applyActivity(s, act(3, { type: "textDelta", text: "Reading" }));
    expect(again).toBe(s);
  });

  it("says a finished turn's answer once when its live pieces say it too", () => {
    // Opening the chat of an agent that has just finished: its words are still held.
    const detail: AgentSessionDetail = {
      session: session(),
      turns: [{ ...finished("Hello!"), objective: "Say hello" }],
      activity: [
        act(1, { type: "textDelta", text: "Hello!" }),
        act(2, { type: "message", text: "Hello!" }),
      ],
    };
    const t = turn(fromDetail(detail));
    expect(t.parts.map((p) => (p.kind === "text" ? p.text : p.kind))).toEqual(["Hello!"]);
    expect(t.state).toBe("done");
    // Its answer, when no piece says it, is still there.
    const bare = turn(fromDetail({ ...detail, activity: [] }));
    expect(bare.parts.map((p) => (p.kind === "text" ? p.text : p.kind))).toEqual(["Hello!"]);
  });

  it("shows a finished turn's steps from what was kept, after a restart (ADR-203)", () => {
    // Plenipo no longer holds the live pieces: it sends what the record kept instead, whole
    // messages and steps, numbered by step as live pieces are.
    const detail: AgentSessionDetail = {
      session: session(),
      turns: [{ ...finished("Saved plan.md."), objective: "Write a plan" }],
      activity: [
        act(1, { type: "sessionStarted", providerSessionId: "p", model: "sonnet" }),
        act(2, { type: "toolUse", tool: "write_file", summary: "plan.md", id: "c1" }),
        act(3, { type: "toolResult", tool: null, isError: false, summary: "saved", id: "c1" }),
        act(4, { type: "message", text: "Saved plan.md." }),
      ],
    };
    const t = turn(fromDetail(detail));
    expect(t.state).toBe("done");
    expect(t.parts.map((p) => p.kind)).toEqual(["tools", "text"]);
    const tools = t.parts[0];
    expect(tools?.kind === "tools" && tools.calls.map((c) => [c.summary, c.state])).toEqual([
      ["plan.md", "done"],
    ]);
    // The answer is said once.
    expect(t.parts.filter((p) => p.kind === "text")).toHaveLength(1);
  });

  it("adds the answer after what was kept when its last message was not kept", () => {
    // A big step keeps at most so many pieces: its last message may be missing.
    const detail: AgentSessionDetail = {
      session: session(),
      turns: [{ ...finished("All done: the tests pass."), objective: "Fix it" }],
      activity: [
        act(1, { type: "message", text: "Let me look at the tests." }),
        act(2, { type: "toolUse", tool: "bash", summary: "npm test", id: "c1" }),
      ],
    };
    const t = turn(fromDetail(detail));
    expect(t.parts.map((p) => (p.kind === "text" ? p.text : p.kind))).toEqual([
      "Let me look at the tests.",
      "tools",
      "All done: the tests pass.",
    ]);
  });

  it("adds no answer joined from messages already on screen, nor one said with other spacing", () => {
    const joined: AgentSessionDetail = {
      session: session(),
      turns: [{ ...finished("Part one.\n\nPart two."), objective: "Go" }],
      activity: [
        act(1, { type: "message", text: "Part one." }),
        act(2, { type: "toolUse", tool: "bash", summary: "ls", id: "c1" }),
        act(3, { type: "message", text: "Part two." }),
      ],
    };
    expect(
      turn(fromDetail(joined)).parts.map((p) => (p.kind === "text" ? p.text : p.kind)),
    ).toEqual(["Part one.", "tools", "Part two."]);
    const spaced: AgentSessionDetail = {
      session: session(),
      turns: [{ ...finished("Hello!"), objective: "Say hello" }],
      activity: [act(1, { type: "message", text: "Hello!\n" })],
    };
    expect(turn(fromDetail(spaced)).parts.filter((p) => p.kind === "text")).toHaveLength(1);
  });
});

describe("what it is doing now", () => {
  const phrase = (tool: string) => toolPhrase(tool, true);

  it("names the running tool with what it was given", () => {
    const s = run([{ type: "toolUse", tool: "mcp__plenipo__run_command", summary: "cargo test" }]);
    expect(doingNow(turn(s), phrase)).toMatchObject({
      text: "Running a program",
      detail: "cargo test",
    });
  });

  it("says thinking, waiting, writing, and starting", () => {
    expect(doingNow(turn(run([{ type: "reasoning", text: "hm" }])), phrase)?.text).toBe("Thinking");
    expect(
      doingNow(
        turn(run([{ type: "status", phase: "waiting", text: "Trying again (2 of 10)." }])),
        phrase,
      )?.text,
    ).toBe("Trying again (2 of 10).");
    expect(doingNow(turn(run([{ type: "textDelta", text: "Hi" }])), phrase)?.text).toBe("Writing");
    expect(doingNow(turn(applyTurn(emptySession(SESSION), record())), phrase)?.text).toBe(
      "Starting",
    );
    const waiting = applyTurn(emptySession(SESSION), record({ running: false, waiting: true }));
    expect(doingNow(turn(waiting), phrase)?.text).toBe("Waiting for its team to answer");
  });

  it("is nothing once the turn is over", () => {
    const s = applyTurn(emptySession(SESSION), finished("ok"));
    expect(doingNow(turn(s), phrase)).toBeNull();
  });
});

describe("files", () => {
  it("lists what was saved or changed, each once", () => {
    const s = run([
      { type: "toolUse", tool: "mcp__plenipo__write_file", summary: "clear-temp.ps1" },
      { type: "toolUse", tool: "mcp__plenipo__read_file", summary: "README.md" },
      { type: "toolUse", tool: "mcp__plenipo__edit_file", summary: "clear-temp.ps1" },
      { type: "toolUse", tool: "mcp__plenipo__write_file", summary: "notes.md" },
    ]);
    expect(filesOf(turn(s))).toEqual([
      { path: "clear-temp.ps1", changed: true, failed: false },
      { path: "notes.md", changed: false, failed: false },
    ]);
  });
});

describe("what Plenipo keeps, added to what is on screen", () => {
  it("adds nothing twice when the same pieces come again", () => {
    const pieces = [
      act(1, { type: "textDelta", text: "Saving " }),
      act(2, { type: "textDelta", text: "it now." }),
    ];
    const detail: AgentSessionDetail = {
      session: {
        id: SESSION,
        runtimeId: "claude-code",
        provider: "anthropic",
        providerSessionId: null,
        providerSessionConfirmed: false,
        model: null,
        effort: null,
        title: "Chat",
        state: "open",
        workingDir: "/w",
        createdAt: 1,
        updatedAt: 1,
        turnCount: 1,
        activeTaskId: TASK,
        waitingTaskId: null,
        metadata: {},
      },
      turns: [record()],
      activity: pieces,
    };
    const live = pieces.reduce((s, a) => applyActivity(s, a), emptySession(SESSION));
    const merged = mergeDetail(live, detail);
    const parts = turn(merged).parts;
    expect(parts).toHaveLength(1);
    expect(parts[0]).toMatchObject({ kind: "text", text: "Saving it now." });
    // Merging again changes nothing on screen.
    expect(turn(mergeDetail(merged, detail)).parts).toEqual(parts);
    // Another conversation's record is not this one's.
    expect(mergeDetail(merged, { ...detail, session: { ...detail.session, id: "other" } })).toBe(
      merged,
    );
  });
});
