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

export interface AgentState {
  status: "loading" | "ready" | "error";
  error: string | null;
  runtimes: AgentRuntimeInfo[];
  sessions: Record<string, AgentSession>;
  /** Session IDs, most recently active first. */
  order: string[];
  /** Turns by session ID, in turn order. */
  turns: Record<string, AgentTurn[]>;
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
      const { session, turns, activity } = action.detail;
      let next = upsertSession(state, session);
      const known = state.turns[session.id] ?? [];
      // A turn update may have arrived while the snapshot was in flight: keep the newer one.
      const merged = turns.map((t) => {
        const live = known.find((k) => k.taskId === t.taskId);
        return live && t.running && !live.running ? live : t;
      });
      for (const live of known) {
        if (!merged.some((t) => t.taskId === live.taskId)) merged.push(live);
      }
      next = { ...next, turns: { ...next.turns, [session.id]: sortTurns(merged) } };
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
          const turn = { ...u } as AgentTurn;
          return {
            ...state,
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
