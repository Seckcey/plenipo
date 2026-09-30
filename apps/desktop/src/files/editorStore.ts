/**
 * The files open in the editor (Phase 21, ADR-093 §5), kept while Plenipo's window is open: the
 * list of open files (on this computer, so it comes back after a restart), and each file's editor
 * as it was left — its text, what was typed and not saved, and its undo history — so going to
 * another page and back loses nothing.
 */

import { useSyncExternalStore } from "react";
import type { EditorState } from "@codemirror/state";

import { parseFileKey } from "./refs";

const OPEN_KEY = "plenipo.editor.open";
/** The most files open at once. */
export const MAX_OPEN = 30;

export interface EditorFiles {
  /** Open files' keys, in their tabs' order. */
  keys: readonly string[];
  /** Files with changes not saved yet. */
  unsaved: ReadonlySet<string>;
}

function readKeys(): string[] {
  try {
    const raw = localStorage.getItem(OPEN_KEY);
    const value: unknown = raw === null ? [] : JSON.parse(raw);
    return Array.isArray(value)
      ? value
          .filter((k): k is string => typeof k === "string" && parseFileKey(k) !== null)
          .slice(0, MAX_OPEN)
      : [];
  } catch {
    return [];
  }
}

/** One window's open files. */
export class EditorStore {
  private files: EditorFiles;
  private states = new Map<string, EditorState>();
  private listeners = new Set<() => void>();

  constructor(private readonly storageKey: string = OPEN_KEY) {
    this.files = { keys: storageKey === OPEN_KEY ? readKeys() : [], unsaved: new Set() };
  }

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  snapshot = () => this.files;

  private set(next: EditorFiles) {
    this.files = next;
    try {
      localStorage.setItem(this.storageKey, JSON.stringify(next.keys));
    } catch {
      // Storage unavailable: the list lasts until the window closes.
    }
    for (const l of this.listeners) l();
  }

  /** Open a file (a tab for it, if it has none). */
  open(key: string) {
    if (this.files.keys.includes(key) || parseFileKey(key) === null) return;
    const keys = [...this.files.keys, key];
    // Too many: the oldest file with nothing unsaved closes.
    while (keys.length > MAX_OPEN) {
      const drop = keys.find((k) => k !== key && !this.files.unsaved.has(k));
      if (!drop) break;
      keys.splice(keys.indexOf(drop), 1);
      this.states.delete(drop);
    }
    this.set({ ...this.files, keys });
  }

  /** Close a file (its changes not saved are dropped). */
  close(key: string) {
    this.states.delete(key);
    const unsaved = new Set(this.files.unsaved);
    unsaved.delete(key);
    this.set({ keys: this.files.keys.filter((k) => k !== key), unsaved });
  }

  /** Keep a file's editor as it is now. */
  keep(key: string, state: EditorState, unsaved: boolean) {
    this.states.set(key, state);
    if (this.files.unsaved.has(key) === unsaved) return;
    const next = new Set(this.files.unsaved);
    if (unsaved) next.add(key);
    else next.delete(key);
    this.set({ ...this.files, unsaved: next });
  }

  /** A file's editor as it was left (`undefined`: none kept). */
  kept(key: string): EditorState | undefined {
    return this.states.get(key);
  }

  /** Forget a file's editor (it is read again from the disk). */
  forget(key: string) {
    this.states.delete(key);
    if (!this.files.unsaved.has(key)) return;
    const next = new Set(this.files.unsaved);
    next.delete(key);
    this.set({ ...this.files, unsaved: next });
  }
}

/** This window's open files. */
export const editorStore = new EditorStore();

export function useEditorFiles(store: EditorStore = editorStore): EditorFiles {
  return useSyncExternalStore(store.subscribe, store.snapshot);
}
