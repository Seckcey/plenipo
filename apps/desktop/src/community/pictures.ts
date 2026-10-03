/**
 * Members' pictures for the cards (Phase 24, ADR-163 §9). A picture is asked for only when the card
 * says it has one, and only once for each member and picture version, so scrolling back through a
 * list never asks again. A picture comes from 8 West and is not trusted: it is shown only as
 * `data:image/png;base64,…` after it is checked to be plain base64 that starts like a PNG file.
 */

import type { CardView } from "@plenipo/types";

import { communityPicture } from "../api/commands";

/** The most pictures kept: one account sees at most 200 cards a day, and a picture can be big. */
const MOST_KEPT = 100;

/** Every PNG file starts with these 8 bytes (`\x89PNG\r\n\x1a\n`), which are this in base64. */
const PNG_START = "iVBORw0KGgo";
const BASE64 = /^[A-Za-z0-9+/]+={0,2}$/;

/** The requests, kept by `memberId:pictureVersion` so cards asking at once share one. */
const asked = new Map<string, Promise<string | null>>();
/** The answers that came, in the same order. */
const known = new Map<string, string | null>();

/** What names a member's picture: a card has them, and so does a place on the leaderboard. */
export type PictureOf = Pick<CardView, "memberId" | "hasPicture" | "pictureVersion">;

/** What to call a card's picture in the cache; `null` when the card has no picture. */
export function pictureKey(card: PictureOf): string | null {
  return card.hasPicture ? `${card.memberId}:${card.pictureVersion ?? ""}` : null;
}

/** The base64 text only when it is safe to put after `data:image/png;base64,`. */
function safePicture(text: string | null): string | null {
  return text !== null && text.startsWith(PNG_START) && BASE64.test(text) ? text : null;
}

/** The picture if it has come already (`null`: there is none), or `undefined` if it has not. */
export function peekPicture(key: string): string | null | undefined {
  return known.get(key);
}

/** Ask for a card's picture, once. A failed request is forgotten, so the next card asks again. */
export function loadPicture(memberId: string, key: string): Promise<string | null> {
  const before = asked.get(key);
  if (before) return before;
  const request: Promise<string | null> = communityPicture(memberId).then(
    (text) => {
      const picture = safePicture(text);
      // Not kept when it was already pushed out while it came.
      if (asked.get(key) === request) known.set(key, picture);
      return picture;
    },
    (reason: unknown) => {
      if (asked.get(key) === request) asked.delete(key);
      throw reason;
    },
  );
  asked.set(key, request);
  // The oldest ones go first when there are too many.
  while (asked.size > MOST_KEPT) {
    const oldest = asked.keys().next();
    if (oldest.done) break;
    asked.delete(oldest.value);
    known.delete(oldest.value);
  }
  return request;
}

/** Forget every picture (the tests start each one with none). */
export function forgetPictures(): void {
  asked.clear();
  known.clear();
}
