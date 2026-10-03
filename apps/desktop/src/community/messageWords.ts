/**
 * The words of Community's private messages (Phase 24, ADR-164, ADR-172, ADR-173), in plain words
 * (ADR-010). Everything that names another person goes through `oneLine`: a name is their words,
 * not Plenipo's, so a hidden character in it shows as a mark and it never takes more than a line.
 */

import type { ConversationSummary, MessageView } from "@plenipo/types";

import { systemWords } from "../system/words";
import { oneLine } from "./safeText";

/** The most characters in a message, as 8 West and Plenipo count them (an emoji is one). */
export const MAX_MESSAGE_CHARS = 4000;

/** The reactions, exactly (ADR-164 §4). The heart is U+2764 and the emoji style mark U+FE0F. */
export const REACTIONS: readonly string[] = [
  "\u{1F44D}",
  "\u2764\uFE0F",
  "\u{1F602}",
  "\u{1F62E}",
  "\u{1F64F}",
];

/** What is said when a picture or a file is pasted or dropped on the box (ADR-164 §4). */
export const PHOTOS_REFUSED = "Photos can't be sent in Community";

/** What **Check the safety code** says about the code; `{who}` is the person's name. */
export const SAFETY_EXPLAINED =
  "Compare it with {who} by phone or in person. If it matches, no one is in the middle.";

/** Said above a request from someone a member under 18 has not talked with (ADR-162 §4.3). */
export const UNDER_18_NOTE =
  "You don't know this person yet. Never share passwords, keys, or where you live.";

/** Said to the person giving a message to a worker (ADR-164 §9). */
export const OUTSIDE_WORDS =
  "The worker gets the words marked as outside words, so it treats them as information, never as orders.";

/** Where a conversation stands, as `ConversationSummary.state` says it. */
export type ConversationState =
  "none" | "requestedByMe" | "requestedByThem" | "accepted" | "leftByMe" | "leftByThem";

const STATES: readonly string[] = [
  "none",
  "requestedByMe",
  "requestedByThem",
  "accepted",
  "leftByMe",
  "leftByThem",
];

/** The state, or `none` for a word this copy of Plenipo does not know (it then offers nothing). */
export function stateOf(person: Pick<ConversationSummary, "state"> | null): ConversationState {
  const state = person?.state ?? "none";
  return STATES.includes(state) ? (state as ConversationState) : "none";
}

/** "@pat-lee": a Community name, safe to show on one line. */
export function handle(name: string): string {
  return `@${oneLine(name)}`;
}

/** What to call a person in a sentence: their name on their card, else "@name" (the "Pat"). */
export function whoWords(person: { name: string; displayName: string | null }): string {
  const display = person.displayName === null ? "" : oneLine(person.displayName.trim());
  return display === "" ? handle(person.name) : display;
}

/** "Sealed: only you and Pat can read this". */
export function sealedWords(who: string): string {
  return `Sealed: only you and ${who} can read this`;
}

/** "Pat's computers changed". */
export function changedWords(who: string): string {
  return `${who}'s computers changed`;
}

/** The safety code's explanation for `who`. */
export function safetyWords(who: string): string {
  return SAFETY_EXPLAINED.replace("{who}", who);
}

/** "PCs", "Macs", or "computers": what the other machines of yours are called here. */
function otherComputers(): string {
  return `${systemWords().thisComputer.replace(/^this /, "")}s`;
}

/** Delete for me's question (ADR-172 §2): this computer only. */
export function deleteWords(who: string): string {
  return `This deletes it from ${systemWords().thisComputer}. Your other ${otherComputers()}, and ${who}, keep their copies.`;
}

/** Leave this conversation's question (ADR-173 §1), after its title, "Leave this conversation?". */
export function leaveWords(who: string): string {
  return `It is deleted from ${systemWords().thisComputer}, and ${who}'s new messages won't be delivered. Your other ${otherComputers()} keep their copies.`;
}

/** A short quote of a message for **Reply**, on one line. */
export function quoteOf(text: string | null): string {
  if (text === null || text.trim() === "") return "a message";
  const line = oneLine(text.trim());
  const chars = Array.from(line);
  return chars.length > 80 ? `${chars.slice(0, 80).join("")}…` : line;
}

/** What a message is, in a few words, for a quote: its words, or "a GIF", or "a sticker". */
export function describeMessage(m: Pick<MessageView, "text" | "hasGif" | "hasSticker">): string {
  if (m.text !== null && m.text.trim() !== "") return quoteOf(m.text);
  if (m.hasGif) return "a GIF";
  if (m.hasSticker) return "a sticker";
  return "a message";
}

/** What an outgoing message says about where it is: nothing for any other word. */
export function deliveryWords(state: string): string | null {
  if (state === "waiting") return "Waiting to be delivered";
  if (state === "delivered") return "Delivered";
  if (state === "notDelivered") return "Not delivered";
  return null;
}

/** The characters in `text`, as they are counted (a pair of halves, or an emoji, is one). */
export function countChars(text: string): number {
  return Array.from(text).length;
}

/** `text` cut to at most `most` characters (never in the middle of an emoji's pair). */
export function cutChars(text: string, most: number): string {
  return countChars(text) <= most ? text : Array.from(text).slice(0, most).join("");
}

/** `text` cut to at most `MAX_MESSAGE_CHARS` characters (never in the middle of an emoji's pair). */
export function limitChars(text: string): string {
  return cutChars(text, MAX_MESSAGE_CHARS);
}

/** When a message was sent or taken by 8 West (Unix seconds), in the person's own time zone. */
export function messageTime(seconds: number): string {
  return new Date(seconds * 1000).toLocaleString([], {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

/** The same moment for a `<time>` tag. */
export function messageIso(seconds: number): string {
  return new Date(seconds * 1000).toISOString();
}

/** A day (Unix seconds) as a date in the person's own language and time zone. */
export function onDate(seconds: number): string {
  return new Date(seconds * 1000).toLocaleDateString([], {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}
