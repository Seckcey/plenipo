// Pure state container for the runtime UI. The backend is the source of truth; this state
// is rebuilt from `getRuntimeOverview` + `getExecutionOutput` whenever the view (re)loads,
// and kept live by streamed runtime events.

import type {
  ExecutionOutput,
  ExecutionRecord,
  LaunchProfileInfo,
  OutputLine,
  RuntimeEvent,
  RuntimeOverview,
} from "@plenipo/types";

/** Lines kept per execution in the UI (matches the backend buffer). */
export const MAX_UI_LINES = 1000;
/** Raw events kept for the diagnostics view. */
export const MAX_EVENT_LOG = 100;

export interface OutputView {
  lines: OutputLine[];
  /** Older lines not shown (evicted here or in the backend). */
  dropped: number;
  /** False when the backend no longer has this execution's output. */
  available: boolean;
}

export interface RuntimeState {
  status: "loading" | "ready" | "error";
  error: string | null;
  profiles: LaunchProfileInfo[];
  executions: Record<string, ExecutionRecord>;
  /** Execution IDs, newest first. */
  order: string[];
  outputs: Record<string, OutputView>;
  notices: string[];
  eventLog: { at: number; event: RuntimeEvent }[];
}

export const initialRuntimeState: RuntimeState = {
  status: "loading",
  error: null,
  profiles: [],
  executions: {},
  order: [],
  outputs: {},
  notices: [],
  eventLog: [],
};

export type RuntimeAction =
  | { type: "overviewLoaded"; overview: RuntimeOverview }
  | { type: "loadFailed"; message: string }
  | { type: "outputLoaded"; output: ExecutionOutput }
  | { type: "event"; event: RuntimeEvent; at: number }
  | { type: "recordUpdated"; record: ExecutionRecord };

export function isActive(record: ExecutionRecord): boolean {
  return record.state === "starting" || record.state === "running";
}

export function runtimeReducer(state: RuntimeState, action: RuntimeAction): RuntimeState {
  switch (action.type) {
    case "overviewLoaded": {
      const executions: Record<string, ExecutionRecord> = {};
      for (const record of action.overview.executions) executions[record.id] = record;
      // Keep newer lifecycle info that may have arrived via events during the fetch.
      for (const [id, record] of Object.entries(state.executions)) {
        const fetched = executions[id];
        if (fetched && isActive(fetched) && !isActive(record)) executions[id] = record;
      }
      return {
        ...state,
        status: "ready",
        error: null,
        profiles: action.overview.profiles,
        executions,
        order: action.overview.executions.map((r) => r.id),
        notices: action.overview.notices,
      };
    }
    case "loadFailed":
      return { ...state, status: "error", error: action.message };
    case "outputLoaded": {
      const { executionId, lines, dropped, available } = action.output;
      const current = state.outputs[executionId];
      const merged = mergeLines(current?.lines ?? [], lines);
      return {
        ...state,
        outputs: {
          ...state.outputs,
          [executionId]: {
            lines: merged.lines,
            dropped: Math.max(dropped, current?.dropped ?? 0) + merged.evicted,
            available,
          },
        },
      };
    }
    case "recordUpdated":
      return upsertRecord(state, action.record);
    case "event": {
      const eventLog = [{ at: action.at, event: action.event }, ...state.eventLog].slice(
        0,
        MAX_EVENT_LOG,
      );
      if (action.event.kind === "lifecycle") {
        return { ...upsertRecord(state, action.event.record), eventLog };
      }
      const { executionId, lines } = action.event;
      const current = state.outputs[executionId] ?? { lines: [], dropped: 0, available: true };
      const merged = mergeLines(current.lines, lines);
      return {
        ...state,
        eventLog,
        outputs: {
          ...state.outputs,
          [executionId]: {
            ...current,
            lines: merged.lines,
            dropped: current.dropped + merged.evicted,
          },
        },
      };
    }
  }
}

function upsertRecord(state: RuntimeState, record: ExecutionRecord): RuntimeState {
  const known = record.id in state.executions;
  return {
    ...state,
    executions: { ...state.executions, [record.id]: record },
    order: known ? state.order : [record.id, ...state.order],
  };
}

/** Merge by `seq`: dedupe overlap between a snapshot and live events, keep order, cap size. */
export function mergeLines(
  existing: OutputLine[],
  incoming: OutputLine[],
): { lines: OutputLine[]; evicted: number } {
  if (incoming.length === 0) return { lines: existing, evicted: 0 };
  const last = existing.at(-1)?.seq ?? 0;
  let lines: OutputLine[];
  if (incoming.every((l) => l.seq > last)) {
    lines = existing.concat(incoming);
  } else {
    const bySeq = new Map<number, OutputLine>();
    for (const l of existing) bySeq.set(l.seq, l);
    for (const l of incoming) bySeq.set(l.seq, l);
    lines = [...bySeq.values()].sort((a, b) => a.seq - b.seq);
  }
  const evicted = Math.max(0, lines.length - MAX_UI_LINES);
  return { lines: evicted ? lines.slice(evicted) : lines, evicted };
}
