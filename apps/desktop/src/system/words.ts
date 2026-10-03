import { SYSTEM_WORDS, type SystemWords } from "@plenipo/types";

/**
 * Each system's own words on screen (Phase 23, ADR-155): "this PC" on Windows, "this Mac" on a
 * Mac, "this computer" on Linux. The Rust side decides: its words arrive with the app's
 * information before the first paint (`main.tsx`), so the screens never guess. Until then, and in
 * tests that do not choose a system, Windows' words.
 */
let current: SystemWords = SYSTEM_WORDS.windows;

/** The words for the system Plenipo runs on. */
export function systemWords(): SystemWords {
  return current;
}

/** Use these words from now on (at start-up, and in tests). */
export function setSystemWords(words: SystemWords): void {
  current = words;
}

/** "Windows closed Plenipo", "Your Mac closed Plenipo": a word at the start of a sentence. */
export function sentenceStart(words: string): string {
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/** A key in a shortcut: `ctrl` is the Control key everywhere; `mod` is Ctrl, or Cmd on a Mac. */
export type Key = "ctrl" | "mod" | "shift" | "alt";

const MAC_KEYS: Record<Key, string> = { ctrl: "⌃", mod: "⌘", shift: "⇧", alt: "⌥" };
const PC_KEYS: Record<Key, string> = { ctrl: "Ctrl", mod: "Ctrl", shift: "Shift", alt: "Alt" };

/**
 * A shortcut's label in this system's words: `shortcut(["ctrl", "shift"], "E")` is "Ctrl+Shift+E"
 * on Windows and Linux and "⌃⇧E" on a Mac; `shortcut(["mod"], "S")` is "Ctrl+S", or "⌘S" on a
 * Mac (the keys from `docs/design/vocabulary.md`).
 */
export function shortcut(keys: Key[], last: string): string {
  if (current.system === "mac") return keys.map((k) => MAC_KEYS[k]).join("") + last;
  return [...keys.map((k) => PC_KEYS[k]), last].join("+");
}
