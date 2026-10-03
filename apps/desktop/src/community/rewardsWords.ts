/**
 * The words of rewards (Phase 24, ADR-169, ADR-172), in plain words (ADR-010): Getting started,
 * how to earn points, what a change in your points was for, and your place on the leaderboard.
 * Badges, points, and "Thanked by 12 people" are in `peopleWords.ts`, with the card.
 */

/** What **Getting started** is called, and the steps part 24C shows (ADR-172 §4). */
export const GETTING_STARTED = "Getting started";

/** The leaderboard's own words when it has no place for you, or no one to show. */
export const NOT_ON_BOARD =
  "You aren't on the leaderboard (members under 18 and people who appear offline aren't shown).";

/** Said to a member under 18, whose points are theirs alone (ADR-169 §2). */
export const UNDER_18_BOARD = "Members under 18 aren't on the leaderboard. Your points are below.";

/** Said under **This week**: when a week starts, as 8 West counts it (ADR-169 §2). */
export const WEEK_STARTS = "This week starts on Monday, Pacific time.";

/** A place as words: 1st, 2nd, 3rd, 4th, 11th, 12th, 13th, 21st, 112th, 1,001st. */
export function ordinal(place: number): string {
  const n = Number.isFinite(place) ? Math.max(0, Math.trunc(place)) : 0;
  const lastTwo = n % 100;
  const last = n % 10;
  const ending =
    lastTwo >= 11 && lastTwo <= 13
      ? "th"
      : last === 1
        ? "st"
        : last === 2
          ? "nd"
          : last === 3
            ? "rd"
            : "th";
  return `${n.toLocaleString("en-US")}${ending}`;
}

/** "Your place: 12th". */
export function yourPlaceWords(place: number): string {
  return `Your place: ${ordinal(place)}`;
}

/** What a change in your points was for: the contract's word, and the plain words (ADR-169 §1). */
export const POINT_REASONS: ReadonlyMap<string, string> = new Map([
  ["contact_accepted", "Someone accepted your message request"],
  ["thanks", "Someone thanked you"],
  ["invite_joined", "Someone you invited by email joined"],
  ["link_7_days", "A link lasted 7 days"],
  ["collab_7_days", "A collaboration lasted 7 days"],
  ["getting_started", "You finished Getting started"],
  ["good_month", "A month with no report upheld against you"],
  ["invite_bought_pro", "Someone you invited bought Pro"],
  ["taken_back", "Points taken back"],
  ["removed_by_8west", "Removed by 8 West"],
]);

/** The words for a reason, or `null` for a reason this copy of Plenipo does not know. */
export function pointReasonWords(reason: string): string | null {
  return POINT_REASONS.get(reason) ?? null;
}

/** "+10 points", "+1 point", "-5 points" (with a real minus sign): a change, with its sign. */
export function changeWords(points: number): string {
  const n = Number.isFinite(points) ? Math.trunc(points) : 0;
  const size = Math.abs(n);
  const sign = n > 0 ? "+" : n < 0 ? "\u2212" : "";
  return `${sign}${size.toLocaleString("en-US")} ${size === 1 ? "point" : "points"}`;
}

/** One way to earn points: what happens, and how many points it is worth. */
export interface EarnRow {
  what: string;
  points: number;
}

/** **How to earn points**: ADR-169 §1's table, row for row. */
export const EARN_ROWS: readonly EarnRow[] = [
  { what: "Someone accepts your message request (once for each person)", points: 2 },
  { what: "Someone thanks you", points: 3 },
  { what: "Someone you invited joins Community", points: 5 },
  { what: "A link with another organization lasts 7 days", points: 10 },
  { what: "You help someone as a collaborator for 7 days, or they help you", points: 10 },
  { what: "You finish Getting started (once)", points: 10 },
  { what: "Each month in Community with no report against you upheld", points: 5 },
  { what: "Someone you invited buys Pro and keeps it past 14 days", points: 25 },
];

/** What earns nothing, and the limits (ADR-169 §1). */
export const EARN_LIMITS =
  "Nothing for messages sent, requests sent, invitations sent, links asked for, or time in Plenipo. At most 100 points a week, and 20 a month from any one person. Points have no money value.";

/** Which invitations count (ADR-172 §3): only an email invitation is one. */
export const EARN_INVITES = "Only people you invite by email count.";
