// Pure state container for agent runtimes and sessions. The backend is the source of truth:
// this is rebuilt from `getAgentOverview` / `getAgentSession` and kept live by agent updates.

import type {
  AgentActivity,
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
  return done * 2 + (turn.running ? 1 : 0);
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
      const merged = turns.map((t) => {
        const live = known.find((k) => k.taskId === t.taskId);
        return live && progress(live) > progress(t) ? live : t;
      });
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
      // coalesces streamed text); keep only live items that are newer.
      const byTask = new Map<string, AgentActivity[]>();
      for (const a of activity) byTask.set(a.taskId, [...(byTask.get(a.taskId) ?? []), a]);
      const nextActivity = { ...next.activity };
      for (const [taskId, items] of byTask) {
        const newest = items.at(-1)?.seq ?? 0;
        const newer = (state.activity[taskId] ?? []).filter((a) => a.seq > newest);
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
          if (list.length > 0 && u.seq <= (list.at(-1)?.seq ?? 0)) return state;
          const item = { ...u } as AgentActivity;
          return { ...state, activity: { ...state.activity, [u.taskId]: cap([...list, item]) } };
        }
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

/** One rendered activity row. Consecutive streamed text is joined; a complete message
 * replaces the streamed text it completes. */
export type ActivityItem =
  | { key: string; kind: "streaming"; text: string }
  | { key: string; kind: "event"; activity: AgentActivity };

export function activityItems(activity: AgentActivity[]): ActivityItem[] {
  const items: ActivityItem[] = [];
  // Streamed text not yet followed by its complete message.
  let streamKey = "";
  let streamText = "";
  const flush = (): void => {
    if (streamText) items.push({ key: streamKey, kind: "streaming", text: streamText });
    streamText = "";
  };
  for (const a of activity) {
    const e = a.event;
    if (e.type === "textDelta") {
      if (!streamText) streamKey = `d${a.seq}`;
      streamText += e.text;
      continue;
    }
    if (e.type === "usage") continue; // shown with the result
    if (e.type === "message") {
      streamText = ""; // the message is the complete version of the streamed text
    } else {
      flush();
    }
    items.push({ key: `e${a.seq}`, kind: "event", activity: a });
  }
  flush();
  return items;
}
