/**
 * Line icons for the organization canvas (24×24, stroked with the current color). Roles name
 * their glyph in the role's metadata; unknown names fall back to the generic worker glyph.
 */
const PATHS: Record<string, string[]> = {
  owner: ["M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z", "M4.5 20a7.5 7.5 0 0 1 15 0"],
  organization: [
    "M4 20V8l8-4 8 4v12",
    "M4 20h16",
    "M9 20v-5h6v5",
    "M8.5 10.5h.01M12 10.5h.01M15.5 10.5h.01",
  ],
  executive: ["M4 18h16", "M5 18 4 8l4.5 3.5L12 5l3.5 6.5L20 8l-1 10"],
  manager: ["M4 8h16v11H4z", "M9 8V6a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2", "M4 13h16", "M11 13v2h2v-2"],
  coordinator: ["M6 21V4", "M6 4h11l-2.5 4L17 12H6"],
  code: ["m8 8-4 4 4 4", "m16 8 4 4-4 4", "m13.5 5-3 14"],
  review: ["M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14Z", "m20 20-4-4", "m8 11 2 2 4-4"],
  qa: [
    "M9 4h6v3H9z",
    "M9 5.5H6.5A1.5 1.5 0 0 0 5 7v12.5A1.5 1.5 0 0 0 6.5 21h11a1.5 1.5 0 0 0 1.5-1.5V7a1.5 1.5 0 0 0-1.5-1.5H15",
    "m8.5 14 2.5 2.5 4.5-5",
  ],
  shield: ["M12 3 5 6v5.5c0 4.3 3 7.8 7 9.5 4-1.7 7-5.2 7-9.5V6z", "m9 12 2 2 4-4"],
  docs: ["M7 3h7l5 5v13H7z", "M14 3v5h5", "M10 13h6M10 17h6"],
  research: [
    "M4 5h6a2 2 0 0 1 2 2v13a2 2 0 0 0-2-2H4z",
    "M20 5h-6a2 2 0 0 0-2 2v13a2 2 0 0 1 2-2h6z",
  ],
  design: [
    "M12 21a9 9 0 1 1 9-9c0 2-1.5 3-3.5 3H16a2 2 0 0 0-1.5 3.3c.6.8.1 2.7-2.5 2.7Z",
    "M7.5 11h.01M10 7.5h.01M14.5 7.5h.01",
  ],
  web: [
    "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z",
    "M3.5 9h17M3.5 15h17",
    "M12 3c2.5 2.5 3.5 5.5 3.5 9s-1 6.5-3.5 9c-2.5-2.5-3.5-5.5-3.5-9s1-6.5 3.5-9Z",
  ],
  server: ["M4 4h16v6H4z", "M4 14h16v6H4z", "M7.5 7h.01M7.5 17h.01", "M11 7h5M11 17h5"],
  worker: ["M13 3 5 13.5h6L10 21l8-10.5h-6z"],
  plus: ["M12 5v14M5 12h14"],
  minus: ["M5 12h14"],
  fit: ["M4 9V4h5", "M20 9V4h-5", "M4 15v5h5", "M20 15v5h-5"],
  link: [
    "M9 15 15 9",
    "M10.5 6.5 12 5a4.2 4.2 0 0 1 6 6l-1.5 1.5",
    "M13.5 17.5 12 19a4.2 4.2 0 0 1-6-6l1.5-1.5",
  ],
  close: ["m6 6 12 12M18 6 6 18"],
  search: ["M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14Z", "m20 20-4-4"],
  grip: ["M9 6h.01M9 12h.01M9 18h.01M15 6h.01M15 12h.01M15 18h.01"],
  // The canvas toolbar and the legend (Phase 18).
  select: ["M5 3l13 7-6 1.5L9 18z", "m12.5 12.5 5 5"],
  hand: [
    "M8 13V5.5a1.5 1.5 0 0 1 3 0V12",
    "M11 11V4.5a1.5 1.5 0 0 1 3 0V12",
    "M14 11.5V6a1.5 1.5 0 0 1 3 0v8a7 7 0 0 1-7 7h-.5a6 6 0 0 1-4.6-2.2L3 16a1.6 1.6 0 0 1 2.4-2L8 16",
  ],
  arrange: ["M12 3v18M3 12h18", "m9 6 3-3 3 3", "m9 18 3 3 3-3", "m6 9-3 3 3 3", "m18 9 3 3-3 3"],
  tidy: ["M4 4h6v6H4z", "M14 4h6v6h-6z", "M4 14h6v6H4z", "M14 14h6v6h-6z"],
  filter: ["M4 5h16l-6 7.5V19l-4 1.5v-8z"],
  legend: ["M4 6h.01M4 12h.01M4 18h.01", "M8 6h12M8 12h12M8 18h12"],
  where: [
    "M12 21s-7-6.2-7-11.5a7 7 0 0 1 14 0C19 14.8 12 21 12 21Z",
    "M12 12a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5Z",
  ],
  trash: ["M4 7h16", "M9 7V4.5h6V7", "M6.5 7l1 13h9l1-13", "M10 11v6M14 11v6"],
  stop: ["M7 7h10v10H7z"],
  help: [
    "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z",
    "M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6",
    "M12 17h.01",
  ],
  watch: [
    "M2.5 12S6 5 12 5s9.5 7 9.5 7-3.5 7-9.5 7-9.5-7-9.5-7Z",
    "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z",
  ],
  lent: ["M4 12h11", "m11 8 4 4-4 4", "M19 5v14"],
  home: ["M3 11l9-7 9 7", "M5 9.5V20h14V9.5", "M10 20v-6h4v6"],
  cloud: ["M7 18a4.5 4.5 0 0 1-.6-9A6 6 0 0 1 18 8.5 4.8 4.8 0 0 1 17.5 18z"],
  pc: ["M3 5h18v11H3z", "M8 20h8", "M12 16v4"],
  folder: [
    "M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2h8.5A1.5 1.5 0 0 1 21 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z",
  ],
  screen: ["M3 4h18v12H3z", "M12 16v4", "M8 20h8", "m10 8 4 2-4 2z"],
  undo: ["M9 14 4 9l5-5", "M4 9h10.5a5.5 5.5 0 0 1 0 11H11"],
};

export function Glyph({
  name,
  size = 18,
  className,
}: {
  name: string;
  size?: number;
  className?: string;
}) {
  const paths = PATHS[name] ?? PATHS.worker ?? [];
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {paths.map((d) => (
        <path key={d} d={d} />
      ))}
    </svg>
  );
}
