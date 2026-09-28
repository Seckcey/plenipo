/**
 * A Watch tab for code (Phase 18, ADR-055): the files one agent's workers changed in its latest
 * objective, and which of them the tab shows. What Plenipo had when the tab opened
 * (`get_watch`) and each change as it happens (`plenipo://watch`) are merged here the way
 * Plenipo's Watch hub keeps them: each file's latest change, newest first, and a new objective
 * starts a fresh list. **Follow along** shows the newest change; **Pin this file** keeps one file
 * in view. Pure: no React, and nothing here writes anywhere.
 */

import type { ChangeKind, WatchChange, WatchState, WatchUpdate, WatchView } from "@plenipo/types";
import type { Status } from "@plenipo/ui";

/** The most files a tab keeps in its list (the oldest go first). */
export const MAX_FILES = 300;

export interface CodeWatch {
  positionId: string;
  /** The objective the list belongs to (the root task), once one is known. */
  objectiveTaskId: string | null;
  /** Each file's latest change, newest first. */
  changes: readonly WatchChange[];
  /** The text so far of each change still being written (or waiting), by change ID. */
  writing: Readonly<Record<string, string>>;
  /** Some changes are from before Plenipo started again: their lines are not kept. */
  fromTheRecord: boolean;
  /** Follow along (the newest change shows), or Pin this file (`pinned` shows). */
  following: boolean;
  /** The pinned file's path. */
  pinned: string | null;
}

export function emptyCodeWatch(positionId: string): CodeWatch {
  return {
    positionId,
    objectiveTaskId: null,
    changes: [],
    writing: {},
    fromTheRecord: false,
    following: true,
    pinned: null,
  };
}

/** Whether a change may still grow text: being written, or waiting for your approval. */
export function isOpen(state: WatchState): boolean {
  return state === "writing" || state === "waiting";
}

/**
 * Put one change in the list. `writing` is the text so far: a string, `null` for none, or
 * `undefined` when unknown (a change read from `get_watch`), which keeps what the tab has.
 * A change older than the one the list has for its file, or from an older objective, is news
 * that came late: it changes nothing.
 */
function put(state: CodeWatch, change: WatchChange, writing: string | null | undefined): CodeWatch {
  let s = state;
  if (s.objectiveTaskId !== null && change.objectiveTaskId !== s.objectiveTaskId) {
    const newest = s.changes[0];
    if (newest && change.at < newest.at) return state;
    // A new objective: a fresh list, following along.
    s = {
      ...s,
      changes: [],
      writing: {},
      fromTheRecord: false,
      following: true,
      pinned: null,
    };
  }
  const i = s.changes.findIndex((c) => c.path === change.path);
  const old = i >= 0 ? s.changes[i] : undefined;
  if (old && old.at > change.at) return state;
  const rest = s.changes.filter((_, j) => j !== i);
  // Newest first; a change as new as another goes before it (it was heard later).
  const at = rest.findIndex((c) => c.at <= change.at);
  const changes = at < 0 ? [...rest, change] : [...rest.slice(0, at), change, ...rest.slice(at)];
  const text: Record<string, string> = { ...s.writing };
  if (old && old.id !== change.id) delete text[old.id];
  if (!isOpen(change.state) || writing === null) delete text[change.id];
  else if (writing !== undefined) text[change.id] = writing;
  while (changes.length > MAX_FILES) {
    const dropped = changes.pop();
    if (dropped) delete text[dropped.id];
  }
  return { ...s, objectiveTaskId: change.objectiveTaskId, changes, writing: text };
}

/** A change as it happens: kept when it is this agent's. */
export function applyWatchUpdate(state: CodeWatch, update: WatchUpdate): CodeWatch {
  if (update.change.positionId !== state.positionId) return state;
  return put(state, update.change, update.writing ?? null);
}

/**
 * What Plenipo had when the tab opened, merged with what the tab already heard since it started
 * listening (listen first, then read: nothing is missed, and nothing goes back in time).
 */
export function loadWatchView(state: CodeWatch, view: WatchView): CodeWatch {
  if (view.positionId !== state.positionId) return state;
  let s = state;
  // Oldest first, so the newest decides the objective.
  for (const change of [...view.changes].reverse()) {
    s = put(s, { ...change, positionId: change.positionId ?? view.positionId }, undefined);
  }
  if (view.fromTheRecord && s.objectiveTaskId === (view.objectiveTaskId ?? null)) {
    s = { ...s, fromTheRecord: true };
  }
  return s;
}

/** The change the tab shows: the pinned file's, or the newest. */
export function shownChange(state: CodeWatch): WatchChange | null {
  if (!state.following && state.pinned !== null) {
    const pinned = state.changes.find((c) => c.path === state.pinned);
    if (pinned) return pinned;
  }
  return state.changes[0] ?? null;
}

/** Pin this file: the tab keeps showing `path` as changes arrive elsewhere. */
export function pinFile(state: CodeWatch, path: string): CodeWatch {
  return { ...state, following: false, pinned: path };
}

/** Follow along (on: the newest change shows), or pin the file shown now (off). */
export function setFollowing(state: CodeWatch, on: boolean): CodeWatch {
  if (on) return { ...state, following: true, pinned: null };
  const shown = shownChange(state);
  return shown ? pinFile(state, shown.path) : { ...state, following: false };
}

/** The newest change (its conversation is the one Stop stops). */
export function latestChange(state: CodeWatch): WatchChange | null {
  return state.changes[0] ?? null;
}

/** Whether a worker is writing a change now (for the tab's mark). */
export function writingNow(state: CodeWatch): boolean {
  return state.changes.some((c) => c.state === "writing");
}

// ---- Words ---------------------------------------------------------------------------------

/** Each state's word, as the owner reads it (docs/design/vocabulary.md). */
export const STATE_WORD: Record<WatchState, string> = {
  writing: "being written — not saved yet",
  waiting: "waiting for your approval",
  saved: "saved",
  refused: "refused",
  notSaved: "not saved",
};

/** Each state's mark (a shape and a color, always beside its word). */
export const STATE_STATUS: Record<WatchState, Status> = {
  writing: "pending",
  waiting: "warn",
  saved: "ok",
  refused: "error",
  notSaved: "offline",
};

export const KIND_WORD: Record<ChangeKind, string> = {
  created: "created",
  changed: "changed",
};

function plural(n: number, one: string): string {
  return `${n} ${one}${n === 1 ? "" : "s"}`;
}

/** "3 lines added, 1 removed" (what the "+3 −1" beside a file says). */
export function countsWords(change: Pick<WatchChange, "added" | "removed">): string {
  return `${plural(change.added, "line")} added, ${change.removed} removed`;
}

/** "3 lines removed here". */
export function removedWords(n: number): string {
  return `${plural(n, "line")} removed here`;
}

/** The file's name and the folder it is in: `src/app.rs` → `["app.rs", "src/"]`. */
export function splitPath(path: string): [name: string, folder: string] {
  const cut = path.lastIndexOf("/");
  return cut < 0 ? [path, ""] : [path.slice(cut + 1), path.slice(0, cut + 1)];
}

/**
 * "Task 2" for each change, when the list holds the work of more than one task (an on-call
 * agent brings in a worker for each task): numbered in the order the tasks first changed a file.
 */
export function taskLabels(changes: readonly WatchChange[]): Map<string, string> {
  const order: string[] = [];
  for (const c of [...changes].sort((a, b) => a.at - b.at)) {
    if (!order.includes(c.taskId)) order.push(c.taskId);
  }
  const labels = new Map<string, string>();
  if (order.length > 1) order.forEach((id, i) => labels.set(id, `Task ${i + 1}`));
  return labels;
}
