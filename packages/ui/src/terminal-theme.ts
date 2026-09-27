/**
 * The terminal's look (Phase 12, ADR-031) for xterm.js, from the design tokens: its colors
 * follow the theme, and its text uses Plenipo's monospace font at the base size.
 */

import { FONT_FAMILY, FONT_SIZE, PALETTES, TERMINAL_COLORS, type ThemeName } from "./tokens";

/** xterm.js's names for the 16 colors, in the same order as `TERMINAL_COLORS`. */
const XTERM_NAMES = [
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "white",
  "brightBlack",
  "brightRed",
  "brightGreen",
  "brightYellow",
  "brightBlue",
  "brightMagenta",
  "brightCyan",
  "brightWhite",
] as const;

export type TerminalTheme = Record<
  | (typeof XTERM_NAMES)[number]
  | "background"
  | "foreground"
  | "cursor"
  | "cursorAccent"
  | "selectionBackground",
  string
>;

/** The terminal's colors in `theme`. */
export function terminalTheme(theme: ThemeName): TerminalTheme {
  const p = PALETTES[theme];
  const colors = Object.fromEntries(
    XTERM_NAMES.map((name, i) => [name, p[TERMINAL_COLORS[i] ?? "terminal-fg"]]),
  ) as Record<(typeof XTERM_NAMES)[number], string>;
  return {
    ...colors,
    background: p["terminal-bg"],
    foreground: p["terminal-fg"],
    cursor: p["terminal-cursor"],
    cursorAccent: p["terminal-bg"],
    selectionBackground: p["terminal-selection"],
  };
}

/** The terminal's font: Plenipo's monospace family, at the base text size. */
export const TERMINAL_FONT = { family: FONT_FAMILY.mono, size: FONT_SIZE.md } as const;
