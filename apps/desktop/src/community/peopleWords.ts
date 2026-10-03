/**
 * The words on a card in Community's directory (Phase 24, ADR-163 §4, ADR-169): the badges, the
 * points, and "Thanked by 12 people". What the account service sends is a word of its own
 * (`founding_member`, `top_helper`); the screen shows the plain words, and ignores a word this
 * copy of Plenipo does not know.
 */

import type { Mood, OwnerStatus } from "@plenipo/types";

import { MOOD_WORDS, STATUS_WORDS } from "../owner/words";

/** The badges: the contract's words (`Badge`) and the words shown for each (ADR-169 §3). */
export const BADGE_WORDS: ReadonlyMap<string, string> = new Map([
  ["founding_member", "Founding member"],
  ["helper", "Helper"],
  ["connector", "Connector"],
  ["good_neighbor", "Good neighbor"],
  ["trusted", "Trusted"],
  ["top_helper", "Top helper this week"],
]);

/** The one-line reason for each badge, said when someone points at it (ADR-169 §3). */
export const BADGE_REASONS: ReadonlyMap<string, string> = new Map([
  ["founding_member", "Joined Community in its first 90 days"],
  ["helper", "Someone's collaborator for 30 days or more"],
  ["connector", "3 links with other organizations, each 30 days or more"],
  ["good_neighbor", "Thanked by 10 or more different people"],
  ["trusted", "In Community 6 months with no report upheld"],
  ["top_helper", "Most points last week"],
]);

/** The words for a badge, or `null` for a badge this copy of Plenipo does not know. */
export function badgeWords(badge: string): string | null {
  return BADGE_WORDS.get(badge) ?? null;
}

/** The one-line reason for a badge, or `null` for a badge this copy of Plenipo does not know. */
export function badgeReason(badge: string): string | null {
  return BADGE_REASONS.get(badge) ?? null;
}

/** A number of points as words: "0 points", "1 point", "1,250 points". */
export function pointsWords(points: number): string {
  const n = Number.isFinite(points) ? Math.max(0, Math.trunc(points)) : 0;
  return `${n.toLocaleString("en-US")} ${n === 1 ? "point" : "points"}`;
}

/** "Thanked by 1 person", "Thanked by 12 people"; `null` when no one has thanked them. */
export function thankedWords(count: number): string | null {
  const n = Number.isFinite(count) ? Math.trunc(count) : 0;
  if (n < 1) return null;
  return `Thanked by ${n.toLocaleString("en-US")} ${n === 1 ? "person" : "people"}`;
}

/**
 * What a card can say of a status. Busy, away, and available get a light (the tile's own); a
 * member who appears offline is **Offline**, in words only. Any other word is ignored.
 */
export type CardStatus = { kind: "light"; status: OwnerStatus } | { kind: "offline" };

export function cardStatus(word: string | null): CardStatus | null {
  switch (word) {
    case "available":
    case "busy":
    case "away":
      return { kind: "light", status: word };
    case "offline":
      return { kind: "offline" };
    default:
      return null;
  }
}

/** The words for a light on a card (the tile's own words). */
export function lightWords(status: OwnerStatus): string {
  return STATUS_WORDS[status];
}

/** A mood word this copy of Plenipo knows (`Object.hasOwn`: `constructor` is not a mood). */
export function knownMood(word: string | null): Mood | null {
  return word !== null && Object.prototype.hasOwnProperty.call(MOOD_WORDS, word)
    ? (word as Mood)
    : null;
}
