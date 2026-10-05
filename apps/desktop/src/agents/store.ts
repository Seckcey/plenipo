// Pure state container for agent runtimes and sessions. The backend is the source of truth:
// this is rebuilt from `getAgentOverview` / `getAgentSession` and kept live by agent updates.

import type {
  AgentActivity,
  AgentEvent,
  AgentOverview,
  AgentRuntimeInfo,
  AgentSession,
  AgentSessionDetail,
  AgentTurn,
  AgentUpdate,
} from "@plenipo/types";

/** Live activity items kept per turn in the UI. */
export const MAX_ACTIVITY = 1000;

/** Activity of step `n` of a turn is numbered from `(n - 1) * STEP_SEQ + 1` (mirrors Core). */
export const STEP_SEQ = 1_000_000;

/** The step (1, 2, …) an activity sequence number belongs to. */
export function stepOf(seq: number): number {
  return Math.floor((Math.max(seq, 1) - 1) / STEP_SEQ) + 1;
}

/** The longest streamed text joined into one item, as in Core's buffer; more starts another. */
const MAX_JOINED_TEXT = 64 * 1024;
/** The most pieces joined into one item; more starts another. */
const MAX_JOINED_PIECES = 1000;

/**
 * Streamed pieces joined into one item, one right after another: the first piece's `seq`, where
 * each piece ends in the text, and when the newest piece came. A snapshot that already has some
 * of them takes the rest.
 */
interface Pieces {
  from: number;
  ends: number[];
  last: number;
}

/** An activity item as the store keeps it: streamed pieces joined (see `joinPiece`). */
type Kept = AgentActivity & { pieces?: Pieces };

/** The words or thinking a streamed piece carries; `null` for anything else. */
function streamed(a: AgentActivity): string | null {
  const e = a.event;
  return e.type === "textDelta" || e.type === "reasoning" ? e.text : null;
}

/** The `seq` of an item's first piece: it names the item's row while the item grows. */
function firstSeq(a: Kept): number {
  return a.pieces?.from ?? a.seq;
}

/**
 * When an item's newest piece came: what an agent "said last" (an item's own `ts` is when its
 * first piece came, as in Core's buffer).
 */
export function saidAt(a: AgentActivity): number {
  return (a as Kept).pieces?.last ?? a.ts;
}

/**
 * A live piece added to the item before it, when both are streamed words or both are streamed
 * thinking, and it comes right after it in the same step: one item for each paragraph, as Core's
 * buffer keeps them, so a long answer or thought never pushes the turn's earlier steps out. The
 * item takes the newest piece's `seq`, keeps its first piece's time (when the thinking began, as
 * after a reload), and its first piece's `seq` names its row.
 */
function joinPiece(last: Kept, next: AgentActivity): Kept | null {
  const before = streamed(last);
  const piece = streamed(next);
  if (before === null || piece === null || last.event.type !== next.event.type) return null;
  const pieces = last.pieces ?? { from: last.seq, ends: [before.length], last: last.ts };
  if (
    next.seq !== last.seq + 1 ||
    stepOf(next.seq) !== stepOf(last.seq) ||
    before.length + piece.length > MAX_JOINED_TEXT ||
    pieces.ends.length >= MAX_JOINED_PIECES
  ) {
    return null;
  }
  const text = before + piece;
  return {
    ...next,
    ts: last.ts,
    event: { ...next.event, text } as AgentEvent,
    pieces: { from: pieces.from, ends: [...pieces.ends, text.length], last: next.ts },
  };
}

/**
 * What a kept item adds to a snapshot that is complete up to `newest`: all of it, nothing, or,
 * for pieces joined across that point, only the pieces after it.
 */
function newerThan(a: Kept, newest: number): Kept[] {
  if (a.seq <= newest) return [];
  const p = a.pieces;
  if (!p || p.from > newest) return [a];
  const cut = p.ends[newest - p.from] ?? 0;
  const ends = p.ends.slice(newest - p.from + 1).map((end) => end - cut);
  const text = (streamed(a) ?? "").slice(cut);
  const pieces = { from: newest + 1, ends, last: p.last };
  return [{ ...a, event: { ...a.event, text } as AgentEvent, pieces }];
}

export interface AgentState {
  status: "loading" | "ready" | "error";
  error: string | null;
  runtimes: AgentRuntimeInfo[];
  sessions: Record<string, AgentSession>;
  /** Session IDs, most recently active first. */
  order: string[];
  /** Turns by session ID, in turn order. */
  turns: Record<string, AgentTurn[]>;
  /** Sessions whose full history has been fetched (live updates alone are partial). */
  loaded: Record<string, true>;
  /** Activity by task ID, in `seq` order. */
  activity: Record<string, AgentActivity[]>;
  notices: string[];
}

export const initialAgentState: AgentState = {
  status: "loading",
  error: null,
  runtimes: [],
  sessions: {},
  order: [],
  turns: {},
  loaded: {},
  activity: {},
  notices: [],
};

export type AgentAction =
  | { type: "overviewLoaded"; overview: AgentOverview }
  | { type: "loadFailed"; message: string }
  | { type: "sessionLoaded"; detail: AgentSessionDetail }
  | { type: "runtimesLoaded"; runtimes: AgentRuntimeInfo[] }
  | { type: "update"; update: AgentUpdate };

export function isRunning(session: AgentSession | undefined): boolean {
  return Boolean(session?.activeTaskId);
}

/** A turn is waiting to continue, e.g. for handoff replies. It holds no worker slot. */
export function isWaiting(session: AgentSession | undefined): boolean {
  return Boolean(session?.waitingTaskId);
}

/** Liaison settings Core stored with a session (`metadata.liaison`). */
export interface LiaisonSessionInfo {
  /** The worker may hand off work through Liaison. */
  enabled: boolean;
  /**
   * `owner` for sessions the owner started, `handoff` for handoff workers, `member` for the agent
   * of an organization position (given objectives from the Organization view).
   */
  origin: "owner" | "handoff" | "member" | null;
  parentTaskId: string | null;
  parentSessionId: string | null;
  depth: number | null;
  /** The organization position the session works for, if any. */
  positionId: string | null;
}

export function liaisonInfo(session: AgentSession | undefined): LiaisonSessionInfo {
  const raw = session?.metadata?.liaison;
  const l = typeof raw === "object" && raw !== null ? (raw as Record<string, unknown>) : {};
  const text = (v: unknown): string | null => (typeof v === "string" && v !== "" ? v : null);
  const origin = text(l.origin);
  const rawWorkforce = session?.metadata?.workforce;
  const w =
    typeof rawWorkforce === "object" && rawWorkforce !== null
      ? (rawWorkforce as Record<string, unknown>)
      : {};
  return {
    enabled: l.enabled === true,
    origin: origin === "owner" || origin === "handoff" || origin === "member" ? origin : null,
    parentTaskId: text(l.parentTaskId),
    parentSessionId: text(l.parentSessionId),
    depth: typeof l.depth === "number" ? l.depth : null,
    positionId: text(w.positionId),
  };
}

/** How far a turn has got; it only ever moves forward. Steps run, wait, run again, finish. */
function progress(turn: AgentTurn): number {
  if (turn.result) return Number.MAX_SAFE_INTEGER;
  const done = turn.steps.filter((s) => s.result).length;
  // "Running" counts only while a step is unfinished (or none is recorded yet).
  const unfinished = turn.steps.length === 0 || turn.steps.some((s) => !s.result);
  return done * 2 + (turn.running && unfinished ? 1 : 0);
}

/** The newer of what is live and a snapshot of the same turn. */
function newer(live: AgentTurn | undefined, snapshot: AgentTurn): AgentTurn {
  if (!live) return snapshot;
  const [l, s] = [progress(live), progress(snapshot)];
  if (l !== s) return l > s ? live : snapshot;
  // Equally far: a snapshot never stops what is live from having stopped running.
  return snapshot.running && !live.running ? live : snapshot;
}

/** A session's running / waiting markers follow its newest known turns. */
function syncSession(session: AgentSession, turns: AgentTurn[]): AgentSession {
  if (turns.length === 0) return session;
  const activeTaskId = turns.find((t) => t.running)?.taskId ?? null;
  const waitingTaskId = turns.find((t) => !t.running && t.waiting)?.taskId ?? null;
  return activeTaskId === session.activeTaskId && waitingTaskId === session.waitingTaskId
    ? session
    : { ...session, activeTaskId, waitingTaskId };
}

export function agentReducer(state: AgentState, action: AgentAction): AgentState {
  switch (action.type) {
    case "overviewLoaded": {
      const sessions: Record<string, AgentSession> = {};
      for (const s of action.overview.sessions) sessions[s.id] = s;
      return {
        ...state,
        status: "ready",
        error: null,
        runtimes: action.overview.runtimes,
        sessions,
        order: action.overview.sessions.map((s) => s.id),
        notices: action.overview.notices,
      };
    }
    case "loadFailed":
      return { ...state, status: "error", error: action.message };
    case "runtimesLoaded":
      return { ...state, runtimes: action.runtimes };
    case "sessionLoaded": {
      const { turns, activity } = action.detail;
      // A command's snapshot can arrive after newer live updates (e.g. a fast turn finished
      // before `start` returned): never let it move facts backwards.
      const session = mergeSession(state.sessions[action.detail.session.id], action.detail.session);
      let next = upsertSession(state, session);
      const known = state.turns[session.id] ?? [];
      // A turn update may have arrived while the snapshot was in flight: keep the newer one.
      const merged = turns.map((t) =>
        newer(
          known.find((k) => k.taskId === t.taskId),
          t,
        ),
      );
      for (const live of known) {
        if (!merged.some((t) => t.taskId === live.taskId)) merged.push(live);
      }
      next = {
        ...next,
        sessions: { ...next.sessions, [session.id]: syncSession(session, merged) },
        turns: { ...next.turns, [session.id]: sortTurns(merged) },
        loaded: { ...next.loaded, [session.id]: true },
      };
      // The snapshot is authoritative up to its newest `seq` for each turn (the backend
      // coalesces streamed text); keep only what live items add after it.
      const byTask = new Map<string, AgentActivity[]>();
      for (const a of activity) byTask.set(a.taskId, [...(byTask.get(a.taskId) ?? []), a]);
      const nextActivity = { ...next.activity };
      for (const [taskId, items] of byTask) {
        const newest = items.at(-1)?.seq ?? 0;
        // A finished turn never streams again: what the snapshot has of it is all of it. (Filled
        // in from the record once Plenipo let its live pieces go, it is numbered afresh, so its
        // numbers say nothing about the live ones held here, ADR-203 §10.)
        const finished = turns.some((t) => t.taskId === taskId && t.result !== null);
        const newer = finished
          ? []
          : (state.activity[taskId] ?? []).flatMap((a) => newerThan(a, newest));
        nextActivity[taskId] = cap(items.concat(newer));
      }
      return { ...next, activity: nextActivity };
    }
    case "update": {
      const u = action.update;
      switch (u.kind) {
        case "runtimes":
          return { ...state, runtimes: u.runtimes };
        case "session":
          return upsertSession(state, u);
        case "turn": {
          const list = state.turns[u.sessionId] ?? [];
          const rest = list.filter((t) => t.taskId !== u.taskId);
          const turn = stripKind({ ...u }) as AgentTurn;
          const owner = state.sessions[u.sessionId];
          let sessions = state.sessions;
          if (owner) {
            const next = { ...owner };
            if (turn.running) {
              next.activeTaskId = turn.taskId;
              if (next.waitingTaskId === turn.taskId) next.waitingTaskId = null;
            } else if (turn.waiting) {
              next.waitingTaskId = turn.taskId;
              if (next.activeTaskId === turn.taskId) next.activeTaskId = null;
            } else {
              if (next.activeTaskId === turn.taskId) next.activeTaskId = null;
              if (next.waitingTaskId === turn.taskId) next.waitingTaskId = null;
            }
            if (
              next.activeTaskId !== owner.activeTaskId ||
              next.waitingTaskId !== owner.waitingTaskId
            ) {
              sessions = { ...state.sessions, [u.sessionId]: next };
            }
          }
          return {
            ...state,
            sessions,
            turns: { ...state.turns, [u.sessionId]: sortTurns([...rest, turn]) },
          };
        }
        case "activity": {
          const list = state.activity[u.taskId] ?? [];
          const last = list.at(-1);
          if (last && u.seq <= last.seq) return state;
          const item = { ...u } as AgentActivity;
          const joined = last ? joinPiece(last, item) : null;
          const items = joined ? [...list.slice(0, -1), joined] : cap([...list, item]);
          return { ...state, activity: { ...state.activity, [u.taskId]: items } };
        }
        // How much of a plan an AI tool reported used (ADR-060 §3): the AI tools page reads it
        // from its own command, so nothing here changes.
        case "plan":
          return state;
      }
    }
  }
}

/** Merge a snapshot into what the UI already knows, keeping facts that only move forward. */
function mergeSession(known: AgentSession | undefined, snapshot: AgentSession): AgentSession {
  if (!known) return snapshot;
  const confirmed = known.providerSessionConfirmed && !snapshot.providerSessionConfirmed;
  return {
    ...snapshot,
    providerSessionId: confirmed ? known.providerSessionId : snapshot.providerSessionId,
    providerSessionConfirmed: known.providerSessionConfirmed || snapshot.providerSessionConfirmed,
    model: snapshot.model ?? known.model,
    state: known.state === "closed" ? "closed" : snapshot.state,
    turnCount: Math.max(known.turnCount, snapshot.turnCount),
    updatedAt: Math.max(known.updatedAt, snapshot.updatedAt),
  };
}

function upsertSession(state: AgentState, session: AgentSession): AgentState {
  const plain = stripKind(session);
  return {
    ...state,
    sessions: { ...state.sessions, [plain.id]: plain },
    order: [plain.id, ...state.order.filter((id) => id !== plain.id)],
  };
}

/** Updates carry a `kind` tag; stored objects do not. */
function stripKind<T extends object>(value: T): T {
  if (!("kind" in value)) return value;
  const copy = { ...value } as T & { kind?: string };
  delete copy.kind;
  return copy;
}

function sortTurns(turns: AgentTurn[]): AgentTurn[] {
  return turns.map(stripKind).sort((a, b) => a.number - b.number || a.startedAt - b.startedAt);
}

function cap(items: AgentActivity[]): AgentActivity[] {
  return items.length > MAX_ACTIVITY ? items.slice(items.length - MAX_ACTIVITY) : items;
}

/** One rendered activity row. Consecutive streamed text is joined, and so is consecutive
 * streamed thinking: each is one paragraph that grows. A complete message replaces the streamed
 * text it completes. */
export type ActivityItem =
  | { key: string; kind: "streaming"; text: string }
  | { key: string; kind: "thinking"; text: string }
  | { key: string; kind: "event"; activity: AgentActivity };

export function activityItems(activity: AgentActivity[]): ActivityItem[] {
  const items: ActivityItem[] = [];
  // Streamed text (or thinking) not yet followed by anything else, named by its first piece so
  // its row stays the same row while it grows.
  let runKind: "streaming" | "thinking" = "streaming";
  let runKey = "";
  let runText = "";
  const flush = (): void => {
    if (runText) items.push({ key: runKey, kind: runKind, text: runText });
    runText = "";
  };
  for (const [i, a] of activity.entries()) {
    const e = a.event;
    // Claude Code's sign that it began to think says nothing more once its thinking's words
    // follow: the thinking row says it. Without words (thinking not shown), the sign stays.
    if (
      e.type === "status" &&
      e.phase === "thinking" &&
      activity[i + 1]?.event.type === "reasoning"
    ) {
      continue;
    }
    if (e.type === "textDelta" || e.type === "reasoning") {
      const kind = e.type === "textDelta" ? "streaming" : "thinking";
      if (kind !== runKind) flush();
      if (!runText) {
        runKind = kind;
        runKey = `${kind === "streaming" ? "d" : "t"}${firstSeq(a)}`;
      }
      runText += e.text;
      continue;
    }
    if (e.type === "usage") continue; // shown with the result
    // A step just starting shows until its call is complete (the live conversation's last
    // line); it never splits the words being typed.
    if (e.type === "status" && e.phase === "starting") continue;
    if (e.type === "message" && runKind === "streaming") {
      runText = ""; // the message is the complete version of the streamed text
    } else {
      flush();
    }
    items.push({ key: `e${a.seq}`, kind: "event", activity: a });
  }
  flush();
  return items;
}
