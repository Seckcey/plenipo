/**
 * Plenipo's own line icons: simple strokes on a 24-unit grid, drawn in the current text color.
 * No UniFi (or other) artwork (ADR-030 §5).
 */

export interface Shape {
  /** Path data, strokes only. */
  d: string;
  /** Circles: [cx, cy, r]. */
  circles?: [number, number, number][];
}

export const ICONS = {
  home: { d: "M3 11l9-7 9 7M5 9.5V20h14V9.5M10 20v-6h4v6" },
  organization: { d: "M9 3h6v5H9zM3 16h6v5H3zM15 16h6v5h-6zM12 8v4M6 16v-4h12v4" },
  department: {
    d: "M4 21V5l8-2v18M12 8h8v13M7 8h2M7 12h2M7 16h2M15 12h2M15 16h2M2 21h20",
  },
  projects: {
    d: "M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2h8.5A1.5 1.5 0 0 1 21 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z",
  },
  workers: {
    d: "M2.5 20a6.5 6.5 0 0 1 13 0M16 4.6a3.3 3.3 0 0 1 0 6.3M18 14.5a5.5 5.5 0 0 1 3.5 5.5",
    circles: [[9, 7.5, 3.5]],
  },
  // Two people side by side: Community, the people who use Plenipo (Phase 24).
  community: {
    d: "M2.5 19a4.5 4.5 0 0 1 9 0M12.5 19a4.5 4.5 0 0 1 9 0",
    circles: [
      [7, 8.5, 2.5],
      [17, 8.5, 2.5],
    ],
  },
  user: { d: "M4 21a8 8 0 0 1 16 0", circles: [[12, 8, 4]] },
  phone: {
    d: "M8 2.5h8A1.5 1.5 0 0 1 17.5 4v16a1.5 1.5 0 0 1-1.5 1.5H8A1.5 1.5 0 0 1 6.5 20V4A1.5 1.5 0 0 1 8 2.5zM10.5 18.5h3",
  },
  approvals: { d: "M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6zM8.5 12l2.5 2.5 4.5-5" },
  shield: { d: "M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z" },
  aiTools: {
    d: "M7 7h10v10H7zM10 10h4v4h-4zM9 3v4M15 3v4M9 17v4M15 17v4M3 9h4M3 15h4M17 9h4M17 15h4",
  },
  activity: { d: "M3 12h4l3-7 4 14 3-7h4" },
  settings: {
    d: "M4 7h9M17 7h3M4 17h3M11 17h9",
    circles: [
      [15, 7, 2],
      [9, 17, 2],
    ],
  },
  diagnostics: { d: "M8 4h8v3H8zM6.5 5.5H5v15h14v-15h-1.5M8 13.5h2l1.5-3 2 6 1.5-3h1" },
  gallery: { d: "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z" },
  list: { d: "M4 6h16M4 12h16M4 18h16" },
  columns: { d: "M4 5h16v14H4zM9.5 5v14M14.5 5v14" },
  bell: { d: "M6 16v-5a6 6 0 1 1 12 0v5l2 2H4zM10 20.5a2 2 0 0 0 4 0" },
  sun: {
    d: "M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4",
    circles: [[12, 12, 4]],
  },
  moon: { d: "M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z" },
  search: { d: "M20 20l-4.5-4.5", circles: [[11, 11, 6]] },
  filter: { d: "M4 5h16l-6 7.5V19l-4 1.5v-8z" },
  chevronDown: { d: "M6 9l6 6 6-6" },
  chevronUp: { d: "M6 15l6-6 6 6" },
  chevronLeft: { d: "M15 6l-6 6 6 6" },
  chevronRight: { d: "M9 6l6 6-6 6" },
  panelClose: { d: "M4 4h16v16H4zM9 4v16M16 9l-3 3 3 3" },
  panelOpen: { d: "M4 4h16v16H4zM9 4v16M13 9l3 3-3 3" },
  close: { d: "M6 6l12 12M18 6L6 18" },
  arrowUp: { d: "M12 19V5M6 11l6-6 6 6" },
  arrowDown: { d: "M12 5v14M6 13l6 6 6-6" },
  sort: { d: "M8 4v16M4 8l4-4 4 4M16 20V4M12 16l4 4 4-4" },
  server: { d: "M4 4h16v6H4zM4 14h16v6H4zM8 7h.01M8 17h.01" },
  globe: {
    d: "M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18",
    circles: [[12, 12, 9]],
  },
  terminal: { d: "M3 5h18v14H3zM7 10l3 2-3 2M12 15h5" },
  file: { d: "M6 3h8l4 4v14H6zM14 3v4h4" },
  branch: {
    d: "M6 8v8M18 10a6 6 0 0 1-6 6H8",
    circles: [
      [6, 6, 2],
      [6, 18, 2],
      [18, 8, 2],
    ],
  },
  key: { d: "M11 12l8-8M16 7l2 2M14 9l2 2", circles: [[8, 15, 4]] },
  // A coin with a dollar sign: spending caps (Phase 16 Wave 3).
  spending: {
    d: "M14.5 9.5c-.4-.9-1.4-1.5-2.5-1.5-1.4 0-2.5.8-2.5 2s1.1 1.6 2.5 2 2.5.8 2.5 2-1.1 2-2.5 2c-1.1 0-2.1-.6-2.5-1.5M12 6.5v11",
    circles: [[12, 12, 9]],
  },
  lock: { d: "M6 11h12v9H6zM8.5 11V8a3.5 3.5 0 0 1 7 0v3" },
  clock: { d: "M12 7v5l3 2", circles: [[12, 12, 9]] },
  alert: { d: "M12 4l9 16H3zM12 10v4M12 17h.01" },
  info: { d: "M12 11v5M12 8h.01", circles: [[12, 12, 9]] },
  check: { d: "M5 12.5l4.5 4.5L19 7" },
  stop: { d: "M7 7h10v10H7z" },
  play: { d: "M8 5l11 7-11 7z" },
  plus: { d: "M12 5v14M5 12h14" },
  more: { d: "M5 12h.01M12 12h.01M19 12h.01" },
  external: { d: "M14 4h6v6M20 4l-9 9M18 14v6H4V6h6" },
  link: {
    d: "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
  },
  refresh: { d: "M20 11a8 8 0 1 0-2.3 5.7M20 5v6h-6" },
  lesson: { d: "M9 18h6M10 21h4M12 3a6 6 0 0 0-3.5 10.9V16h7v-2.1A6 6 0 0 0 12 3z" },
} satisfies Record<string, Shape>;

export type IconName = keyof typeof ICONS;

export const ICON_NAMES = Object.keys(ICONS) as IconName[];
