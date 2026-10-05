/**
 * What a conversation with an agent is made of (ADR-200): the turns of one conversation, each a
 * message from you and the agent's answer built up as the AI tool writes it, in parts — words,
 * thinking, runs of tool calls, and notes. Pure functions only: the screens draw it, the
 * provider feeds it.
 *
 * The AI tool's live events arrive one at a time (`AgentActivity`). Words are joined into one
 * part, not kept piece by piece, so a long answer never grows the list and nothing is dropped.
 */
import type {
  AgentActivity,
  AgentEvent,
  AgentSessionDetail,
  AgentTurn,
  TokenUsage,
  TurnOutcome,
} from "@plenipo/types";

import { turnUsage } from "../agents/format";
import { stepOf } from "../agents/store";
import { toolKind } from "./words";

export type ToolState = "running" | "done" | "failed";

export interface ToolCall {
  id: string;
  /** The tool as the AI tool names it (Claude Code: `mcp__plenipo__write_file`). */
  tool: string;
  /** What it was given, in one line (a path, or a program and its arguments). */
  summary: string;
  state: ToolState;
  /** What came back, in one line. */
  result: string;
  startedAt: number;
  endedAt: number | null;
}

export type Part =
  | { kind: "text"; id: string; text: string; streaming: boolean }
  | { kind: "thinking"; id: string; text: string; startedAt: number; endedAt: number | null }
  | { kind: "tools"; id: string; calls: ToolCall[] }
  | { kind: "note"; id: string; level: "info" | "warning"; text: string };

export type TurnState = "working" | "waiting" | "done" | "stopped" | "failed";

export interface ChatTurn {
  taskId: string;
  /** The turn's place in its conversation, 1 for the first. */
  number: number;
  /** What was asked (your words, or the lead's). Empty until the turn's record arrives. */
  ask: string;
  startedAt: number;
  endedAt: number | null;
  state: TurnState;
  parts: Part[];
  /** What the AI tool is waiting for now (a retry, a pause), until something else happens. */
  status: { text: string; at: number } | null;
  /** The final answer, once the task has finished. */
  answer: string | null;
  /** Why it did not finish, in one line. */
  problem: string | null;
  outcome: TurnOutcome | null;
  /** Its tokens, added up over its steps (each step reports its own). */
  usage: TokenUsage | null;
  /** Stopped before a step reported its tokens: it used at least `usage`. */
  usageAtLeast: boolean;
  /** What each step reported live, by step, until the turn's record adds them up. */
  stepUsage: Record<number, TokenUsage>;
  model: string | null;
  /** The newest piece of live activity applied (replays of older ones are ignored). */
  seq: number;
}

export interface ChatSession {
  sessionId: string;
  turns: ChatTurn[];
}

export function emptySession(sessionId: string): ChatSession {
  return { sessionId, turns: [] };
}

function newTurn(taskId: string, number: number, at: number): ChatTurn {
  return {
    taskId,
    number,
    ask: "",
    startedAt: at,
    endedAt: null,
    state: "working",
    parts: [],
    status: null,
    answer: null,
    problem: null,
    outcome: null,
    usage: null,
    usageAtLeast: false,
    stepUsage: {},
    model: null,
    seq: 0,
  };
}

function addUsage(a: TokenUsage, b: TokenUsage): TokenUsage {
  return {
    inputTokens: a.inputTokens + b.inputTokens,
    cachedInputTokens: a.cachedInputTokens + b.cachedInputTokens,
    outputTokens: a.outputTokens + b.outputTokens,
  };
}

/**
 * A turn's tokens from its record: added up over its steps (the last step's alone for a turn
 * recorded before steps were). `atLeast` when it was stopped and a step reported none.
 */
export function turnTokens(record: AgentTurn): { usage: TokenUsage | null; atLeast: boolean } {
  const usage = turnUsage(record);
  const outcome = record.result?.outcome;
  const stopped = outcome === "cancelled" || outcome === "interrupted";
  return {
    usage,
    atLeast: usage !== null && stopped && record.steps.some((s) => !s.result?.usage),
  };
}

/** Is the turn over (done, stopped, or failed)? */
export function isOver(turn: ChatTurn): boolean {
  return turn.state === "done" || turn.state === "stopped" || turn.state === "failed";
}

/** Is the conversation busy: a turn that has not finished? */
export function isBusy(session: ChatSession): boolean {
  return session.turns.some((t) => !isOver(t));
}

/**
 * The turns whose own record, as Plenipo has it now, says they are over without saying how: not
 * running, not waiting, and no result. Only these are stuck; a turn the record says is running or
 * waiting is live, whatever the session's summary says.
 */
export function stuckTurns(records: readonly AgentTurn[]): Set<string> {
  return new Set(records.filter((r) => !r.running && !r.waiting && !r.result).map((r) => r.taskId));
}

/**
 * Turns whose end was missed (`stuck`, from their records) show as stopped, so the chat stops
 * showing them as running. Every other turn is left as it is. A real end replaces this if it comes.
 */
export function endUnfinished(
  session: ChatSession,
  at: number,
  stuck: ReadonlySet<string>,
): ChatSession {
  if (!session.turns.some((t) => !isOver(t) && stuck.has(t.taskId))) return session;
  return {
    ...session,
    turns: session.turns.map((t) =>
      isOver(t) || !stuck.has(t.taskId)
        ? t
        : {
            ...t,
            state: "stopped",
            endedAt: t.endedAt ?? at,
            parts: closeOpen(t.parts, t.endedAt ?? at, false),
            status: null,
            problem: t.problem ?? "It was no longer running.",
          },
    ),
  };
}

// ---- Parts -------------------------------------------------------------------------------------

let counter = 0;
/** A part's or call's identity on screen: stable once made, never reused. */
function nextId(prefix: string): string {
  counter += 1;
  return `${prefix}-${counter}`;
}

function last<T>(items: readonly T[]): T | undefined {
  return items[items.length - 1];
}

/** Close what was open: a thinking block ends, and words that were being written are done. */
function closeOpen(parts: Part[], at: number, keepText: boolean): Part[] {
  return parts.map((p) => {
    if (p.kind === "thinking" && p.endedAt === null) return { ...p, endedAt: at };
    if (!keepText && p.kind === "text" && p.streaming) return { ...p, streaming: false };
    return p;
  });
}

function withLast(parts: Part[], update: (p: Part) => Part): Part[] {
  const out = parts.slice();
  const i = out.length - 1;
  const p = out[i];
  if (p) out[i] = update(p);
  return out;
}

/** Where the last part of a kind is, or -1. */
function lastIndexOfKind(parts: readonly Part[], kind: Part["kind"]): number {
  for (let i = parts.length - 1; i >= 0; i -= 1) if (parts[i]?.kind === kind) return i;
  return -1;
}

/** The last words that are still being written, if any. */
function lastStreamingText(parts: readonly Part[]): number {
  for (let i = parts.length - 1; i >= 0; i -= 1) {
    const p = parts[i];
    if (p?.kind === "text" && p.streaming) return i;
  }
  return -1;
}

function applyEvent(turn: ChatTurn, event: AgentEvent, at: number, step = 1): ChatTurn {
  let parts = turn.parts;
  switch (event.type) {
    case "sessionStarted":
      return event.model ? { ...turn, model: event.model } : turn;
    case "textDelta": {
      const tail = last(parts);
      if (tail?.kind === "text" && tail.streaming) {
        return {
          ...turn,
          status: null,
          parts: withLast(parts, (p) =>
            p.kind === "text" ? { ...p, text: p.text + event.text } : p,
          ),
        };
      }
      parts = closeOpen(parts, at, false);
      return {
        ...turn,
        status: null,
        parts: [...parts, { kind: "text", id: nextId("text"), text: event.text, streaming: true }],
      };
    }
    case "message": {
      parts = closeOpen(parts, at, true);
      const open = lastStreamingText(parts);
      if (open >= 0) {
        // The whole message replaces the pieces that were written so far.
        const next = parts.slice();
        const p = next[open];
        if (p?.kind === "text") next[open] = { ...p, text: event.text, streaming: false };
        return { ...turn, status: null, parts: next };
      }
      const tail = last(parts);
      if (tail?.kind === "text" && tail.text === event.text)
        return { ...turn, status: null, parts };
      return {
        ...turn,
        status: null,
        parts: [...parts, { kind: "text", id: nextId("text"), text: event.text, streaming: false }],
      };
    }
    case "reasoning": {
      const tail = last(parts);
      if (tail?.kind === "thinking" && tail.endedAt === null) {
        return {
          ...turn,
          status: null,
          parts: withLast(parts, (p) =>
            p.kind === "thinking" ? { ...p, text: p.text + event.text } : p,
          ),
        };
      }
      parts = closeOpen(parts, at, false);
      return {
        ...turn,
        status: null,
        parts: [
          ...parts,
          {
            kind: "thinking",
            id: nextId("thinking"),
            text: event.text,
            startedAt: at,
            endedAt: null,
          },
        ],
      };
    }
    case "toolUse": {
      parts = closeOpen(parts, at, false);
      const call: ToolCall = {
        id: event.id ?? nextId("call"),
        tool: event.tool,
        summary: event.summary,
        state: "running",
        result: "",
        startedAt: at,
        endedAt: null,
      };
      const tail = last(parts);
      if (tail?.kind === "tools") {
        return {
          ...turn,
          status: null,
          parts: withLast(parts, (p) =>
            p.kind === "tools" ? { ...p, calls: [...p.calls, call] } : p,
          ),
        };
      }
      return {
        ...turn,
        status: null,
        parts: [...parts, { kind: "tools", id: nextId("tools"), calls: [call] }],
      };
    }
    case "toolResult": {
      // The result belongs to the call with its ID, or else to the oldest call still running.
      for (let i = parts.length - 1; i >= 0; i -= 1) {
        const p = parts[i];
        if (p?.kind !== "tools") continue;
        let at_ = p.calls.findIndex((c) => event.id !== undefined && c.id === event.id);
        if (at_ < 0) at_ = p.calls.findIndex((c) => c.state === "running");
        if (at_ < 0) continue;
        const next = parts.slice();
        const calls = p.calls.slice();
        const call = calls[at_];
        if (!call) continue;
        calls[at_] = {
          ...call,
          state: event.isError ? "failed" : "done",
          result: event.summary,
          endedAt: at,
        };
        next[i] = { ...p, calls };
        return { ...turn, status: null, parts: next };
      }
      return { ...turn, status: null };
    }
    case "notice":
      return {
        ...turn,
        parts: [
          ...closeOpen(parts, at, true),
          {
            kind: "note",
            id: nextId("note"),
            level: event.level === "warning" ? "warning" : "info",
            text: event.text,
          },
        ],
      };
    case "usage": {
      // Each step reports its own: the turn's tokens are theirs added up.
      const stepUsage = { ...turn.stepUsage, [step]: event.usage };
      return { ...turn, stepUsage, usage: Object.values(stepUsage).reduce(addUsage) };
    }
    case "memoryShortened":
      return {
        ...turn,
        parts: [
          ...closeOpen(parts, at, true),
          { kind: "note", id: nextId("note"), level: "info", text: event.detail },
        ],
      };
    case "status":
      return { ...turn, status: { text: event.text, at } };
    case "plan":
      // The worker's plan (Phase 25, item 3.1) is for the live conversation's progress line;
      // the chat's own plan comes from its to-do list (PlanPanel).
      return turn;
  }
}

// ---- A conversation ----------------------------------------------------------------------------

function indexOfTurn(session: ChatSession, taskId: string): number {
  return session.turns.findIndex((t) => t.taskId === taskId);
}

function replaceTurn(session: ChatSession, i: number, turn: ChatTurn): ChatSession {
  const turns = session.turns.slice();
  turns[i] = turn;
  return { ...session, turns };
}

/** One piece of live activity, applied to its turn (a turn not seen before is begun). */
export function applyActivity(session: ChatSession, activity: AgentActivity): ChatSession {
  if (activity.sessionId !== session.sessionId) return session;
  const i = indexOfTurn(session, activity.taskId);
  const turn = i >= 0 ? session.turns[i] : undefined;
  const base = turn ?? newTurn(activity.taskId, session.turns.length + 1, activity.ts);
  // A piece that was applied already (it came in the replay and again live) is skipped.
  if (turn && activity.seq <= turn.seq) return session;
  const next = {
    ...applyEvent(base, activity.event, activity.ts, stepOf(activity.seq)),
    seq: activity.seq,
  };
  if (i >= 0) return replaceTurn(session, i, next);
  return { ...session, turns: [...session.turns, next] };
}

function stateOf(turn: AgentTurn): TurnState {
  const result = turn.result;
  if (!result) return turn.waiting && !turn.running ? "waiting" : "working";
  switch (result.outcome) {
    case "completed":
      return "done";
    case "cancelled":
    case "interrupted":
      return "stopped";
    default:
      return "failed";
  }
}

/**
 * A turn's record (its message, its state, its final answer), applied to the conversation. The
 * answer is shown even if some of the live pieces were missed: when no words on screen say it,
 * it is added.
 */
export function applyTurn(session: ChatSession, record: AgentTurn): ChatSession {
  if (record.sessionId !== session.sessionId) return session;
  const i = indexOfTurn(session, record.taskId);
  const existing = i >= 0 ? session.turns[i] : undefined;
  const base = existing ?? newTurn(record.taskId, record.number, record.startedAt);
  const state = stateOf(record);
  const answer = record.result?.text ?? null;
  let parts = base.parts;
  if (state !== "working" && state !== "waiting") {
    parts = closeOpen(parts, record.endedAt ?? base.startedAt, false);
    // Said already: the same words, give or take the space around them.
    const said = parts.some(
      (p) => p.kind === "text" && answer !== null && p.text.trim() === answer.trim(),
    );
    if (answer && answer.trim() !== "" && !said) {
      // The pieces on screen do not end with the answer: the last words are replaced by it when
      // they are its beginning, and it is added after them when they are not. When the answer
      // ends with them, it was joined from several messages that are all on screen already.
      // (A turn filled in from its record may lack its last message: a step keeps at most so
      // many pieces, ADR-203 §10. Its answer is then added after what was kept.)
      const open = lastIndexOfKind(parts, "text");
      const tail = parts[open];
      const last = open >= 0 && tail?.kind === "text" ? tail : null;
      if (last && answer.startsWith(last.text.slice(0, 40))) {
        parts = parts.slice();
        parts[open] = { ...last, text: answer, streaming: false };
      } else if (!last?.text.trim() || !answer.trim().endsWith(last.text.trim())) {
        parts = [...parts, { kind: "text", id: nextId("text"), text: answer, streaming: false }];
      }
    }
  }
  const tokens = turnTokens(record);
  const usage = tokens.usage ?? base.usage;
  const next: ChatTurn = {
    ...base,
    number: record.number,
    ask: record.objective,
    startedAt: record.startedAt,
    endedAt: record.endedAt,
    state,
    parts,
    status: state === "working" || state === "waiting" ? base.status : null,
    answer,
    problem:
      state === "failed" || state === "stopped"
        ? (record.result?.error ?? record.result?.summary ?? null)
        : null,
    outcome: record.result?.outcome ?? null,
    usage,
    usageAtLeast: tokens.usage ? tokens.atLeast : base.usageAtLeast,
    model: record.result?.model ?? base.model,
  };
  const turns = i >= 0 ? session.turns.slice() : [...session.turns, next];
  if (i >= 0) turns[i] = next;
  turns.sort((a, b) => a.number - b.number || a.startedAt - b.startedAt);
  return { ...session, turns };
}

/**
 * What Plenipo keeps now (the turns, and the live pieces it still has), added to a conversation
 * already on screen. Pieces it shows already are skipped, so nothing appears twice.
 */
export function mergeDetail(start: ChatSession, detail: AgentSessionDetail): ChatSession {
  if (detail.session.id !== start.sessionId) return start;
  let session = start;
  // Each turn's record first (its place, and what was asked), not yet its end: its answer is
  // added at the end only if its own pieces have not said it, so it is never said twice.
  for (const turn of detail.turns) session = applyTurn(session, { ...turn, result: null });
  // Each turn's pieces in their own order (the numbers count within a turn).
  const ordered = detail.activity.slice().sort((a, b) => a.seq - b.seq);
  for (const activity of ordered) session = applyActivity(session, activity);
  // The live pieces were applied after the records, so a finished turn keeps its final state.
  for (const turn of detail.turns) session = applyTurn(session, turn);
  return session;
}

/** A conversation from what Plenipo keeps now: its turns, and the live pieces it still has. */
export function fromDetail(detail: AgentSessionDetail): ChatSession {
  return mergeDetail(emptySession(detail.session.id), detail);
}

// ---- What to show now --------------------------------------------------------------------------

/** What an agent is doing right now, in plain words, and since when. */
export interface Doing {
  /** "Running a program", "Thinking", "Waiting for Claude's AI company". */
  text: string;
  /** What it is working on (a file, a program and its arguments), when there is one. */
  detail: string;
  since: number;
}

export function doingNow(turn: ChatTurn, phrase: (tool: string) => string): Doing | null {
  if (isOver(turn)) return null;
  const tail = last(turn.parts);
  if (turn.status) return { text: turn.status.text, detail: "", since: turn.status.at };
  if (tail?.kind === "tools") {
    const running = tail.calls.find((c) => c.state === "running");
    if (running) {
      return { text: phrase(running.tool), detail: running.summary, since: running.startedAt };
    }
  }
  if (tail?.kind === "thinking" && tail.endedAt === null) {
    return { text: "Thinking", detail: "", since: tail.startedAt };
  }
  if (turn.state === "waiting") {
    return { text: "Waiting for its team to answer", detail: "", since: turn.startedAt };
  }
  if (tail?.kind === "text" && tail.streaming) {
    return { text: "Writing", detail: "", since: turn.startedAt };
  }
  if (turn.parts.length === 0) {
    return { text: "Starting", detail: "", since: turn.startedAt };
  }
  return { text: "Working", detail: "", since: turn.startedAt };
}

/** The files a turn saved or changed, newest last, each once (by what the tool was given). */
export function filesOf(turn: ChatTurn): { path: string; changed: boolean; failed: boolean }[] {
  const seen = new Map<string, { path: string; changed: boolean; failed: boolean }>();
  for (const part of turn.parts) {
    if (part.kind !== "tools") continue;
    for (const call of part.calls) {
      const kind = toolKind(call.tool);
      if (kind !== "write" && kind !== "edit") continue;
      const path = call.summary.trim().split(/\s+/)[0] ?? "";
      if (path === "") continue;
      seen.set(path, { path, changed: kind === "edit", failed: call.state === "failed" });
    }
  }
  return [...seen.values()];
}
