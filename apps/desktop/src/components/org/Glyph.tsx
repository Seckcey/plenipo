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
