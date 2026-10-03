import { systemWords } from "../system/words";
/**
 * Every mark the canvas can show, in one list (Phase 18, ADR-053 §15–§16). The canvas puts each
 * mark's key in a `data-symbol` attribute, and the legend shows this same list, so the two cannot
 * drift apart: a test draws a canvas with every kind of thing and checks each mark it finds has
 * its line here. Each mark has a picture and words; none is told apart by color alone.
 */

export type SymbolGroup = "Tiles" | "Status" | "Lines" | "Labels" | "Badges" | "Where" | "Dragging";

export const SYMBOL_GROUPS: readonly SymbolGroup[] = [
  "Tiles",
  "Status",
  "Lines",
  "Labels",
  "Badges",
  "Where",
  "Dragging",
];

/** How the legend draws a mark's picture (with the canvas's own classes). */
export type SymbolSample =
  | { kind: "tile"; className: string; glyph: string }
  | { kind: "status"; status: string }
  | { kind: "line"; className: string }
  | { kind: "chip"; className: string; text: string }
  | { kind: "badge"; className: string; text: string }
  | { kind: "where"; glyph: string };

export interface CanvasSymbol {
  key: string;
  group: SymbolGroup;
  label: string;
  words: string;
  sample: SymbolSample;
}

export const SYMBOLS = [
  // Tiles
  {
    key: "tile-owner",
    group: "Tiles",
    label: "You",
    words: "Your tile: your picture, status, mood, and message.",
    sample: { kind: "tile", className: "topo-node--owner", glyph: "owner" },
  },
  {
    key: "tile-organization",
    group: "Tiles",
    label: "Your organization",
    words: "Its name and how much work is going on.",
    sample: { kind: "tile", className: "topo-node--organization", glyph: "organization" },
  },
  {
    key: "tile-full-time",
    group: "Tiles",
    label: "Full-time agent",
    words: "One agent holds the job and keeps its conversation. Only full-time agents lead a team.",
    sample: { kind: "tile", className: "topo-node--position", glyph: "manager" },
  },
  {
    key: "tile-on-call",
    group: "Tiles",
    label: "On-call agent",
    words:
      "A new worker is brought in for each task and leaves when it is done; its live workers hang below it.",
    sample: { kind: "tile", className: "topo-node--position topo-node--on-demand", glyph: "code" },
  },
  {
    key: "tile-worker",
    group: "Tiles",
    label: "Live worker",
    words: "A worker an on-call agent brought in for one task, with the task's first line.",
    sample: { kind: "tile", className: "topo-node--worker", glyph: "worker" },
  },
  // Status: a dot and a word
  {
    key: "status-idle",
    group: "Status",
    label: "Idle",
    words: "Ready, with nothing to do.",
    sample: { kind: "status", status: "idle" },
  },
  {
    key: "status-working",
    group: "Status",
    label: "Working",
    words: "Working on a task now (the dot pulses unless motion is reduced).",
    sample: { kind: "status", status: "working" },
  },
  {
    key: "status-waiting",
    group: "Status",
    label: "Waiting on team",
    words: "Waiting for answers from its team.",
    sample: { kind: "status", status: "waiting" },
  },
  {
    key: "status-queued",
    group: "Status",
    label: "Queued",
    words: "Has work lined up, waiting for its turn.",
    sample: { kind: "status", status: "queued" },
  },
  {
    key: "status-vacant",
    group: "Status",
    label: "Vacant",
    words: "A full-time job with nobody in it yet.",
    sample: { kind: "status", status: "vacant" },
  },
  {
    key: "status-unavailable",
    group: "Status",
    label: "Unavailable",
    words: "Cannot work now: no AI tool it may use is ready. Point at it to see why.",
    sample: { kind: "status", status: "unavailable" },
  },
  // Lines
  {
    key: "line-reports",
    group: "Lines",
    label: "Reports to",
    words: "Solid: who each agent reports to. Drag its round end to change it.",
    sample: { kind: "line", className: "topo-link topo-link--tree" },
  },
  {
    key: "line-worker",
    group: "Lines",
    label: "Live worker",
    words: "Short dashes: a live worker and the agent that brought it in.",
    sample: { kind: "line", className: "topo-link topo-link--worker" },
  },
  {
    key: "line-active",
    group: "Lines",
    label: "Work moving",
    words: "Moving dashes along a line: the agent at its end is working.",
    sample: { kind: "line", className: "topo-link topo-link--tree is-active" },
  },
  {
    key: "line-review",
    group: "Lines",
    label: "Review",
    words: "Dotted, labelled Review: a reviewer checks that team's work.",
    sample: { kind: "line", className: "topo-oversight__path topo-oversight__path--review" },
  },
  {
    key: "line-qa",
    group: "Lines",
    label: "QA",
    words: "Dotted, labelled QA: a QA evaluator tests that team's work.",
    sample: { kind: "line", className: "topo-oversight__path topo-oversight__path--qa" },
  },
  {
    key: "line-security",
    group: "Lines",
    label: "Security",
    words: "Dotted, labelled Security: a security auditor checks that team's work.",
    sample: { kind: "line", className: "topo-oversight__path topo-oversight__path--security" },
  },
  {
    key: "line-lent",
    group: "Lines",
    label: "Lent",
    words: "Long dashes, labelled Lent: an agent helping another team for now.",
    sample: { kind: "line", className: "topo-lent__path" },
  },
  {
    key: "line-handoff",
    group: "Lines",
    label: "Hand-off",
    words:
      "An arrow moving from the agent that asked to the agent that took the work, and back with the answer (still, when motion is reduced).",
    sample: { kind: "badge", className: "topo-handoff__mark", text: "➜" },
  },
  // Labels on lines
  {
    key: "chip-department",
    group: "Labels",
    label: "Department",
    words: "On the line to a department's Manager: the department's name.",
    sample: { kind: "chip", className: "topo-chip topo-chip--department", text: "Research" },
  },
  {
    key: "chip-project",
    group: "Labels",
    label: "Project",
    words: "On the line to a project's Supervisor: the project's name.",
    sample: { kind: "chip", className: "topo-chip topo-chip--project", text: "Launch" },
  },
  {
    key: "chip-runtime",
    group: "Labels",
    label: "AI tool",
    words: "On the line to a live worker: the AI tool it uses.",
    sample: { kind: "chip", className: "topo-chip topo-chip--runtime", text: "Claude Code" },
  },
  // Badges on tiles
  {
    key: "badge-oversees",
    group: "Badges",
    label: "Oversees a team",
    words: "Review, QA, or Security, and which team it checks.",
    sample: { kind: "badge", className: "topo-badge topo-badge--review", text: "Review → Shop" },
  },
  {
    key: "badge-lent",
    group: "Badges",
    label: "Lent",
    words: "Helping another team now. Choose it to send the agent home.",
    sample: { kind: "badge", className: "topo-badge topo-badge--lent", text: "Lent → Shop" },
  },
  {
    key: "badge-fixed-tool",
    group: "Badges",
    label: "Fixed AI tool",
    words: "You chose its AI tool; otherwise its role's model choices pick one (Auto).",
    sample: { kind: "badge", className: "topo-badge topo-badge--fixed", text: "Fixed" },
  },
  {
    key: "badge-specialty",
    group: "Badges",
    label: "Specialty",
    words: "What this agent focuses on within its role.",
    sample: { kind: "badge", className: "topo-badge topo-badge--specialty", text: "Payments" },
  },
  {
    key: "badge-experienced",
    group: "Badges",
    label: "Experienced",
    words: "Has learned and done more than your organization's average.",
    sample: { kind: "badge", className: "topo-badge topo-badge--experienced", text: "★" },
  },
  // Where the work is
  {
    key: "where-cloud",
    group: "Where",
    label: "Thinks in an AI company's cloud",
    words: "Its model runs in its AI company's cloud, such as Anthropic's cloud.",
    sample: { kind: "where", glyph: "cloud" },
  },
  {
    key: "where-this-pc",
    group: "Where",
    get label() {
      return `Runs on ${systemWords().thisComputer}`;
    },
    words: "It runs a program, uses Plenipo's browser, or uses the screen here.",
    sample: { kind: "where", glyph: "pc" },
  },
  {
    key: "where-server",
    group: "Where",
    label: "Runs on a server",
    words: "Connected to one of your servers, by name (PRODUCTION when it is).",
    sample: { kind: "where", glyph: "server" },
  },
  {
    key: "touch-folder",
    group: "Where",
    label: "Touching a folder",
    words: "The folder it last read or changed in its working copy.",
    sample: { kind: "where", glyph: "folder" },
  },
  {
    key: "touch-website",
    group: "Where",
    label: "Touching a website",
    words: "The website it has open in Plenipo's browser.",
    sample: { kind: "where", glyph: "web" },
  },
  {
    key: "touch-screen",
    group: "Where",
    label: "Touching the screen",
    words: "It is looking at or using the screen, mouse, and keyboard.",
    sample: { kind: "where", glyph: "screen" },
  },
  // Dragging
  {
    key: "handle",
    group: "Dragging",
    label: "Line end",
    words: "Drag this round end onto another agent to rewire the line.",
    sample: { kind: "badge", className: "topo-handle topo-handle--sample", text: "" },
  },
  {
    key: "toggle",
    group: "Dragging",
    label: "Show or hide a team",
    words: "− hides the team under an agent; + shows it again, with how many are hidden.",
    sample: { kind: "badge", className: "topo-toggle topo-toggle--sample", text: "−" },
  },
  {
    key: "drop-valid",
    group: "Dragging",
    label: "Can drop here",
    words: "A green outline: dropping here opens its choices.",
    sample: { kind: "tile", className: "topo-node--position is-drop-target", glyph: "manager" },
  },
  {
    key: "drop-refused",
    group: "Dragging",
    label: "Cannot drop here",
    words: "A red outline, with the reason in words next to the pointer.",
    sample: { kind: "tile", className: "topo-node--position is-drop-refused", glyph: "manager" },
  },
  {
    key: "faded",
    group: "Dragging",
    label: "Faded tile",
    words: "Shown only so a match's lines make sense, while a filter or a search is on.",
    sample: { kind: "tile", className: "topo-node--position is-dimmed", glyph: "manager" },
  },
] as const satisfies readonly CanvasSymbol[];

export type SymbolKey = (typeof SYMBOLS)[number]["key"];

export const SYMBOL_KEYS: ReadonlySet<string> = new Set(SYMBOLS.map((s) => s.key));

/** The mark for a status dot. */
export function statusSymbol(status: string): SymbolKey | undefined {
  const key = `status-${status}`;
  return SYMBOL_KEYS.has(key) ? (key as SymbolKey) : undefined;
}
