/**
 * Plenipo's design tokens: the one place colors, sizes, and timings are written (ADR-030).
 *
 * `tokens.css` (CSS custom properties) and `tokens.json` (for the Rust side and exports) are
 * generated from this file; `pnpm tokens` rewrites them, and a test fails when they are out of
 * date. Feature code never writes a color value: it uses `var(--ui-…)`.
 */

export type ThemeName = "dark" | "light";

export const THEMES: readonly ThemeName[] = ["dark", "light"];

/** The theme Plenipo starts in. */
export const DEFAULT_THEME: ThemeName = "dark";

/** What each color token is for (the design system document lists these). */
export const COLOR_ROLES = {
  bg: "Application background, behind everything",
  surface: "Panels, cards, tables",
  "surface-raised": "Hover, selected, and floating surfaces above a panel",
  "surface-sunken": "Wells below a panel: logs, terminal output, the map canvas edge",
  "surface-overlay": "Translucent panels floating over the map",
  border: "Thin dividers between rows and around panels",
  "border-strong": "Dividers that must stand out (active panels, table header)",
  "control-border": "Outline of inputs, checkboxes, and switches (3:1 against surfaces)",
  "text-primary": "Titles and body text",
  "text-secondary": "Labels, captions, and supporting text",
  "text-muted": "Timestamps, hints, and placeholders (still meets AA)",
  accent: "The one accent: selection, links, focus, and primary actions",
  "accent-strong": "Accent on hover",
  "accent-fill": "Background of primary buttons",
  "on-accent": "Text on accent-fill",
  "accent-soft": "Tint behind selected rows and active items",
  ok: "Status: working, online, succeeded",
  "ok-text": "Status word in the ok color",
  "ok-soft": "Tint behind an ok notice",
  warn: "Status: needs attention, blocked",
  "warn-text": "Status word in the warn color",
  "warn-soft": "Tint behind a warn notice",
  error: "Status: failed, refused, production",
  "error-text": "Status word and messages in the error color",
  "error-soft": "Tint behind an error notice",
  "on-error": "Text on a solid error background",
  offline: "Status: idle, offline, not running",
  "offline-text": "Status word in the offline color",
  pending: "Status: waiting (for you, or in a queue)",
  "pending-text": "Status word in the pending color",
  "pending-soft": "Tint behind a waiting notice",
  scrim: "Dims the page behind a dialog",
  shadow: "Color of elevation shadows",
  "skeleton-base": "Loading placeholder",
  "skeleton-shine": "Loading placeholder highlight",
  "map-canvas": "Organization map background",
  "map-grid": "Organization map grid lines",
  "map-node": "A tile on the map",
  "map-node-border": "A tile's outline",
  "map-node-hover": "A tile's outline on hover",
  "map-link": "A reporting line in use",
  "map-link-idle": "A reporting line at rest",
  "map-link-glow": "Glow around a live reporting line",
  "map-link-flow": "Work moving along a line",
  "map-chip": "Small label on a tile",
  "rank-vp": "The VP's rank color",
  "rank-manager": "A Manager's rank color",
  "rank-supervisor": "A Supervisor's rank color",
  "role-review": "Oversight: review",
  "role-qa": "Oversight: QA",
  "role-security": "Oversight: security",
  "brand-ink": "Plenipo's logo: the name's letters (docs/brand/pip-brand-kit)",
  "brand-primary": "Plenipo's logo: the P's outer rail and the last o",
  "brand-middle": "Plenipo's logo: the P's middle rail",
  "brand-inner": "Plenipo's logo: the P's inner rail",
  "terminal-bg": "The terminal's background (Phase 12)",
  "terminal-fg": "The terminal's text",
  "terminal-cursor": "The terminal's cursor",
  "terminal-selection": "Text chosen in the terminal",
  "terminal-black": "The terminal's black (a color programs ask for)",
  "terminal-red": "The terminal's red (a color programs ask for)",
  "terminal-green": "The terminal's green (a color programs ask for)",
  "terminal-yellow": "The terminal's yellow (a color programs ask for)",
  "terminal-blue": "The terminal's blue (a color programs ask for)",
  "terminal-magenta": "The terminal's magenta (a color programs ask for)",
  "terminal-cyan": "The terminal's cyan (a color programs ask for)",
  "terminal-white": "The terminal's white (a color programs ask for)",
  "terminal-bright-black": "The terminal's bright black (a color programs ask for)",
  "terminal-bright-red": "The terminal's bright red (a color programs ask for)",
  "terminal-bright-green": "The terminal's bright green (a color programs ask for)",
  "terminal-bright-yellow": "The terminal's bright yellow (a color programs ask for)",
  "terminal-bright-blue": "The terminal's bright blue (a color programs ask for)",
  "terminal-bright-magenta": "The terminal's bright magenta (a color programs ask for)",
  "terminal-bright-cyan": "The terminal's bright cyan (a color programs ask for)",
  "terminal-bright-white": "The terminal's bright white (a color programs ask for)",
  "code-dark": "A picture code's dark squares: the same in both themes, so a phone can read it",
  "code-light": "A picture code's light ground and border: the same in both themes",
} as const;

export type ColorToken = keyof typeof COLOR_ROLES;

type Palette = Record<ColorToken, string>;

const dark: Palette = {
  bg: "#0d0f13",
  surface: "#15181e",
  "surface-raised": "#1c2028",
  "surface-sunken": "#080a0d",
  "surface-overlay": "rgba(21, 24, 30, 0.94)",
  border: "#262b34",
  "border-strong": "#353c48",
  "control-border": "#646f81",
  "text-primary": "#e9edf3",
  "text-secondary": "#a9b2c1",
  "text-muted": "#8d96a6",
  accent: "#4c95ff",
  "accent-strong": "#79b0ff",
  "accent-fill": "#0b63e5",
  "on-accent": "#ffffff",
  "accent-soft": "rgba(76, 149, 255, 0.14)",
  ok: "#3ddc84",
  "ok-text": "#4be391",
  "ok-soft": "rgba(61, 220, 132, 0.12)",
  warn: "#f0b429",
  "warn-text": "#f5c14a",
  "warn-soft": "rgba(240, 180, 41, 0.12)",
  error: "#ff6b6b",
  "error-text": "#ff8f8f",
  "error-soft": "rgba(255, 107, 107, 0.12)",
  "on-error": "#1a0505",
  offline: "#7a8392",
  "offline-text": "#9aa3b1",
  pending: "#b494ff",
  "pending-text": "#c3a9ff",
  "pending-soft": "rgba(180, 148, 255, 0.13)",
  scrim: "rgba(0, 0, 0, 0.62)",
  shadow: "rgba(0, 0, 0, 0.55)",
  "skeleton-base": "#1c2028",
  "skeleton-shine": "#262b34",
  "map-canvas": "#06080b",
  "map-grid": "rgba(140, 165, 210, 0.07)",
  "map-node": "#121620",
  "map-node-border": "#242a36",
  "map-node-hover": "#2d4a78",
  "map-link": "#4c95ff",
  "map-link-idle": "#2a64b8",
  "map-link-glow": "rgba(76, 149, 255, 0.5)",
  "map-link-flow": "#a8d0ff",
  "map-chip": "#0c1a33",
  "rank-vp": "#ffd166",
  "rank-manager": "#7cc4ff",
  "rank-supervisor": "#3ddc97",
  "role-review": "#a78bfa",
  "role-qa": "#f5b83d",
  "role-security": "#ff7a93",
  // The owner's approved brand kit (docs/brand/pip-brand-kit), "on dark" colors.
  "brand-ink": "#f3f7ff",
  "brand-primary": "#5b97ff",
  "brand-middle": "#8cbcff",
  "brand-inner": "#d8e8ff",
  // The terminal (Phase 12): readable colors for what programs print, on its background.
  "terminal-bg": "#080a0d",
  "terminal-fg": "#d6dde6",
  "terminal-cursor": "#4c95ff",
  "terminal-selection": "rgba(76, 149, 255, 0.35)",
  "terminal-black": "#7d8590",
  "terminal-red": "#ff7b72",
  "terminal-green": "#3fb950",
  "terminal-yellow": "#d29922",
  "terminal-blue": "#58a6ff",
  "terminal-magenta": "#bc8cff",
  "terminal-cyan": "#39c5cf",
  "terminal-white": "#b1bac4",
  "terminal-bright-black": "#9198a1",
  "terminal-bright-red": "#ffa198",
  "terminal-bright-green": "#56d364",
  "terminal-bright-yellow": "#e3b341",
  "terminal-bright-blue": "#79c0ff",
  "terminal-bright-magenta": "#d2a8ff",
  "terminal-bright-cyan": "#56d4dd",
  "terminal-bright-white": "#f0f6fc",
  "code-dark": "#000000",
  "code-light": "#ffffff",
};

const light: Palette = {
  bg: "#eef1f5",
  surface: "#ffffff",
  "surface-raised": "#f6f8fa",
  "surface-sunken": "#e9edf2",
  "surface-overlay": "rgba(255, 255, 255, 0.95)",
  border: "#dbe0e7",
  "border-strong": "#c3cad5",
  "control-border": "#7a8698",
  "text-primary": "#121820",
  "text-secondary": "#3d4858",
  "text-muted": "#556072",
  accent: "#0a5dd1",
  "accent-strong": "#084bab",
  "accent-fill": "#0a5dd1",
  "on-accent": "#ffffff",
  "accent-soft": "rgba(10, 93, 209, 0.1)",
  ok: "#0f7236",
  "ok-text": "#0f7236",
  "ok-soft": "rgba(15, 114, 54, 0.1)",
  warn: "#8a5c00",
  "warn-text": "#7d5300",
  "warn-soft": "rgba(180, 120, 0, 0.12)",
  error: "#c62828",
  "error-text": "#b3261e",
  "error-soft": "rgba(198, 40, 40, 0.09)",
  "on-error": "#ffffff",
  offline: "#5d6674",
  "offline-text": "#5d6674",
  pending: "#6a3ccc",
  "pending-text": "#6a3ccc",
  "pending-soft": "rgba(106, 60, 204, 0.09)",
  scrim: "rgba(15, 20, 30, 0.38)",
  shadow: "rgba(15, 25, 40, 0.16)",
  "skeleton-base": "#e6eaf0",
  "skeleton-shine": "#f3f5f8",
  "map-canvas": "#f7f9fc",
  "map-grid": "rgba(30, 50, 90, 0.07)",
  "map-node": "#ffffff",
  "map-node-border": "#cfd6e0",
  "map-node-hover": "#8fb2e8",
  "map-link": "#0a5dd1",
  "map-link-idle": "#4a78c4",
  "map-link-glow": "rgba(10, 93, 209, 0.3)",
  "map-link-flow": "#0a5dd1",
  "map-chip": "#e3edfc",
  "rank-vp": "#8a5c00",
  "rank-manager": "#0f6aad",
  "rank-supervisor": "#0f7236",
  "role-review": "#6a3ccc",
  "role-qa": "#8a5c00",
  "role-security": "#b8264c",
  // The owner's approved brand kit (docs/brand/pip-brand-kit), "on light" colors.
  "brand-ink": "#0b1833",
  "brand-primary": "#2463eb",
  "brand-middle": "#5898f3",
  "brand-inner": "#123c88",
  // The terminal (Phase 12): on white, the white colors are grays, so they stay readable.
  "terminal-bg": "#ffffff",
  "terminal-fg": "#1f2328",
  "terminal-cursor": "#0a5dd1",
  "terminal-selection": "rgba(10, 93, 209, 0.2)",
  "terminal-black": "#24292f",
  "terminal-red": "#cf222e",
  "terminal-green": "#116329",
  "terminal-yellow": "#7d4e00",
  "terminal-blue": "#0969da",
  "terminal-magenta": "#8250df",
  "terminal-cyan": "#1b7c83",
  "terminal-white": "#6e7781",
  "terminal-bright-black": "#57606a",
  "terminal-bright-red": "#a40e26",
  "terminal-bright-green": "#1a7f37",
  "terminal-bright-yellow": "#633c01",
  "terminal-bright-blue": "#0550ae",
  "terminal-bright-magenta": "#6639ba",
  "terminal-bright-cyan": "#1b6d74",
  "terminal-bright-white": "#57606a",
  "code-dark": "#000000",
  "code-light": "#ffffff",
};

/** Every color token, per theme. */
export const PALETTES: Record<ThemeName, Palette> = { dark, light };

/** Type scale (px). The base is 13 px (owner, 2026-09-27); the scale runs 11–20 px. */
export const FONT_SIZE = {
  xs: 11,
  sm: 12,
  md: 13,
  lg: 15,
  xl: 17,
  "2xl": 20,
} as const;

export const FONT_WEIGHT = { regular: 400, medium: 500, semibold: 600 } as const;

export const LINE_HEIGHT = { tight: 1.25, base: 1.45 } as const;

export const FONT_FAMILY = {
  sans: '"Segoe UI Variable", "Segoe UI", system-ui, -apple-system, Roboto, "Helvetica Neue", Arial, sans-serif',
  mono: '"Cascadia Mono", Consolas, "SFMono-Regular", Menlo, "DejaVu Sans Mono", monospace',
} as const;

/** Spacing steps (px). Dense by default: prefer the small steps. */
export const SPACE = {
  "0": 0,
  "1": 2,
  "2": 4,
  "3": 6,
  "4": 8,
  "5": 12,
  "6": 16,
  "7": 20,
  "8": 24,
  "9": 32,
} as const;

/** Corner radius (px): small, 4–8. */
export const RADIUS = { sm: 4, md: 6, lg: 8, pill: 999 } as const;

/** Fixed sizes of the dense layout (px). */
export const SIZE = {
  "row-height": 28,
  "row-height-compact": 24,
  "control-height": 26,
  "rail-width": 88,
  "topbar-height": 44,
  "facet-width": 232,
  "card-width": 240,
  "card-height": 164,
  "dot-size": 8,
  "strip-height": 8,
  "icon-size": 18,
  // Pip, Plenipo's robot: beside an empty page, on Home, and in About.
  "pip-sm": 72,
  "pip-md": 112,
  "pip-lg": 168,
} as const;

/** Elevation: shadow geometry; the color comes from the theme's `shadow` token. */
export const ELEVATION = {
  "1": "0 1px 2px",
  "2": "0 4px 12px",
  "3": "0 12px 32px",
} as const;

/** Motion: durations (ms) and the easing curve. */
export const MOTION = {
  fast: 100,
  base: 160,
  slow: 240,
} as const;

export const EASING = "cubic-bezier(0.2, 0, 0, 1)";

/**
 * Text/background pairs that must meet WCAG AA, checked by a test in both themes. `min` is
 * 4.5 for text and 3 for large text, status marks, and control outlines. A translucent
 * background is laid over `over` first.
 */
export interface ContrastPair {
  fg: ColorToken;
  bg: ColorToken;
  over?: ColorToken;
  min: number;
}

const surfaces: ColorToken[] = ["bg", "surface", "surface-raised", "surface-sunken"];
const texts: ColorToken[] = [
  "text-primary",
  "text-secondary",
  "text-muted",
  "accent",
  "ok-text",
  "warn-text",
  "error-text",
  "offline-text",
  "pending-text",
];
const marks: ColorToken[] = ["ok", "warn", "error", "offline", "pending", "accent"];

const panels: ColorToken[] = ["surface", "surface-raised"];

/** The 16 colors programs ask a terminal for, in their usual order (ANSI 0–15). */
export const TERMINAL_COLORS: readonly ColorToken[] = [
  "terminal-black",
  "terminal-red",
  "terminal-green",
  "terminal-yellow",
  "terminal-blue",
  "terminal-magenta",
  "terminal-cyan",
  "terminal-white",
  "terminal-bright-black",
  "terminal-bright-red",
  "terminal-bright-green",
  "terminal-bright-yellow",
  "terminal-bright-blue",
  "terminal-bright-magenta",
  "terminal-bright-cyan",
  "terminal-bright-white",
];

export const CONTRAST_PAIRS: ContrastPair[] = [
  ...surfaces.flatMap((bg) => texts.map((fg): ContrastPair => ({ fg, bg, min: 4.5 }))),
  ...panels.flatMap((bg) => marks.map((fg): ContrastPair => ({ fg, bg, min: 3 }))),
  ...panels.map((bg): ContrastPair => ({ fg: "control-border", bg, min: 3 })),
  { fg: "on-accent", bg: "accent-fill", min: 4.5 },
  { fg: "on-error", bg: "error", min: 4.5 },
  { fg: "text-primary", bg: "accent-soft", over: "surface", min: 4.5 },
  { fg: "text-secondary", bg: "accent-soft", over: "surface", min: 4.5 },
  { fg: "accent", bg: "accent-soft", over: "surface", min: 4.5 },
  { fg: "ok-text", bg: "ok-soft", over: "surface", min: 4.5 },
  { fg: "warn-text", bg: "warn-soft", over: "surface", min: 4.5 },
  { fg: "error-text", bg: "error-soft", over: "surface", min: 4.5 },
  { fg: "pending-text", bg: "pending-soft", over: "surface", min: 4.5 },
  // Body text on a tinted notice or error panel (laid over the surface).
  ...(["ok-soft", "warn-soft", "error-soft", "pending-soft"] as const).flatMap((bg) =>
    (["text-primary", "text-secondary"] as const).map((fg): ContrastPair => ({
      fg,
      bg,
      over: "surface",
      min: 4.5,
    })),
  ),
  { fg: "text-primary", bg: "surface-overlay", over: "map-canvas", min: 4.5 },
  { fg: "text-secondary", bg: "surface-overlay", over: "map-canvas", min: 4.5 },
  { fg: "text-primary", bg: "map-node", min: 4.5 },
  { fg: "text-secondary", bg: "map-node", min: 4.5 },
  { fg: "text-primary", bg: "map-canvas", min: 4.5 },
  { fg: "text-secondary", bg: "map-canvas", min: 4.5 },
  // Lines that carry meaning (reporting lines, map connectors): 3:1.
  { fg: "map-link-idle", bg: "map-canvas", min: 3 },
  { fg: "map-link", bg: "map-canvas", min: 3 },
  { fg: "control-border", bg: "bg", min: 3 },
  // The terminal: its text and every color programs print, on its background; the cursor.
  { fg: "terminal-fg", bg: "terminal-bg", min: 4.5 },
  ...TERMINAL_COLORS.map((fg): ContrastPair => ({ fg, bg: "terminal-bg", min: 4.5 })),
  { fg: "terminal-cursor", bg: "terminal-bg", min: 3 },
  { fg: "terminal-fg", bg: "terminal-selection", over: "terminal-bg", min: 4.5 },
  // Plenipo's logo: the name's letters are text; the P and the last o are large marks.
  ...(["bg", "surface"] as const).flatMap((bg): ContrastPair[] => [
    { fg: "brand-ink", bg, min: 4.5 },
    { fg: "brand-primary", bg, min: 3 },
  ]),
];

/** The CSS custom property for a token, e.g. `--ui-surface`. */
export const cssVar = (name: string) => `--ui-${name}`;

/** `var(--ui-…)` for a color token, for the rare inline style (charts, SVG fills). */
export const colorVar = (token: ColorToken) => `var(${cssVar(token)})`;
