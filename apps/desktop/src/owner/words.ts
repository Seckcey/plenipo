/**
 * The words for your tile (Phase 18, ADR-056): your status, your mood, and your message. A
 * status is always a light **and** a word; a mood is always a small face **and** a word ("a face
 * alone can be misread"). The faces are decoration: screen readers read the word.
 */

import type { Mood, OwnerProfile, OwnerStatus } from "@plenipo/types";

export const STATUS_ORDER: readonly OwnerStatus[] = ["available", "busy", "away", "doNotDisturb"];

export const STATUS_WORDS: Record<OwnerStatus, string> = {
  available: "Available",
  busy: "Busy",
  away: "Away",
  doNotDisturb: "Do not disturb",
};

export const MOOD_ORDER: readonly Mood[] = [
  "great",
  "good",
  "okay",
  "tired",
  "stressed",
  "focused",
  "celebrating",
];

export const MOOD_WORDS: Record<Mood, string> = {
  great: "Great",
  good: "Good",
  okay: "Okay",
  tired: "Tired",
  stressed: "Stressed",
  focused: "Focused",
  celebrating: "Celebrating",
};

/** A small face for each mood, always shown with its word (and hidden from screen readers). */
export const MOOD_FACES: Record<Mood, string> = {
  great: "😄",
  good: "🙂",
  okay: "😐",
  tired: "😴",
  stressed: "😣",
  focused: "🧐",
  celebrating: "🥳",
};

/** The word for no mood, in the mood picker. */
export const NO_MOOD_WORD = "None";

/** What Do not disturb does, under the status picker. */
export const DO_NOT_DISTURB_HINT =
  "Windows pop-up notices wait while this is on and come as one when you turn it off; the bell still counts them.";

/** While your details are being read: Save waits for them, so nothing kept is lost. */
export const READING_DETAILS = "Reading your details…";

/** When your details could not be read: Save stays off, so nothing kept is lost. */
export function cannotReadDetails(reason: string): string {
  return `Plenipo couldn't read your details, so Save is off for now: ${reason}`;
}

/** Where your picture is kept. */
export const PICTURE_NOTE = "Kept on this PC only. Plenipo never sends it anywhere.";

/** The longest message, as the Ledger keeps it. */
export const MAX_MESSAGE = 80;

/**
 * Your message as one line of plain text, at most `MAX_MESSAGE` long: line breaks, tabs, and
 * other control characters become spaces (the Ledger refuses them), and the end is cut off
 * without breaking a character in two. The length counts as the text box counts it.
 */
export function oneLineMessage(text: string): string {
  let out = "";
  for (const ch of text) {
    const code = ch.codePointAt(0) ?? 0;
    const plain = code < 0x20 || (code >= 0x7f && code <= 0x9f) ? " " : ch;
    if (out.length + plain.length > MAX_MESSAGE) break;
    out += plain;
  }
  return out;
}

/**
 * Your tile in words, for a screen reader or a tooltip: "Available, feeling Great, “Feeling
 * great!”". Empty until your tile is loaded.
 */
export function describeOwner(profile: OwnerProfile | null): string {
  if (!profile) return "";
  const parts = [STATUS_WORDS[profile.status]];
  if (profile.mood) parts.push(`feeling ${MOOD_WORDS[profile.mood]}`);
  if (profile.message) parts.push(`“${profile.message}”`);
  return parts.join(", ");
}
