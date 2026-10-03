/**
 * The words of the notice when a plan runs out (Phase 25, item 4.2; ADR-253). Plain words
 * (ADR-010): what happened, when the work picks back up, and what you can do.
 */
import type { LimitWait } from "@plenipo/types";

import { when } from "../pages/words";

/** Where each company's usage reset is used (the owner's answer 5, ADR-190). */
const RESET_IN: Record<string, string> = { Anthropic: "Claude", OpenAI: "ChatGPT" };

/** "Claude Code is out until 3:00 PM." */
export function limitTitle(w: LimitWait, now: number = Date.now()): string {
  if (w.until === null) return `${w.label}'s usage limit is over.`;
  return w.reported
    ? `${w.label} is out until ${when(w.until, now)}.`
    : `${w.label} reached its usage limit.`;
}

/** What waiting means: when Plenipo picks the work back up by itself. */
export function waitWords(w: LimitWait, now: number = Date.now()): string {
  if (w.until === null) return "Plenipo picks the work back up within a minute.";
  return w.reported
    ? `Plenipo picks the work back up at ${when(w.until, now)}.`
    : `It didn't say when it resets. Plenipo tries again at ${when(w.until, now)} and picks the work back up then.`;
}

/** "2 objectives wait for it:" */
export function waitingLead(w: LimitWait): string {
  const n = w.work.length;
  return n === 1 ? "1 objective waits for it:" : `${n} objectives wait for it:`;
}

/** The reset choice, for a company that gives usage resets; `null` for the others. */
export function resetWords(w: LimitWait): string | null {
  if (!w.resetCompany) return null;
  const place = RESET_IN[w.resetCompany] ?? w.resetCompany;
  return `If ${w.resetCompany} gave you a usage reset, you can use it now in ${place}. Then press Pick it up now.`;
}

/** The choice to use something else: the same company's key, or another AI tool. */
export function otherToolWords(w: LimitWait): string {
  const key = w.resetCompany ? `your ${w.resetCompany} key or ` : "";
  return `Choose ${key}another AI tool for this work in Settings → AI models.`;
}
