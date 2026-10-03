/**
 * The words of **Block**, **Unblock**, **Report**, and **Delete my Community data from this PC**
 * (Phase 24, ADR-167, ADR-168 §3, ADR-173), in plain words (ADR-010). A person's name is their
 * words, not Plenipo's, so it only ever goes through `handle` (`oneLine`): a hidden character
 * shows as a mark and a name never takes more than a line.
 */

import type { ReportReason } from "../api/commands";
import { systemWords } from "../system/words";
import { handle } from "./messageWords";

/** The most characters in a report's note, counted as the composer counts (an emoji is one). */
export const MAX_NOTE_CHARS = 1000;

/** The most messages one report can carry (ADR-167 §6). */
export const MAX_REPORT_MESSAGES = 20;

/** **What is wrong?**, in ADR-167's words and order. `value` is the name the app is sent. */
export const REASONS: readonly { value: ReportReason; label: string }[] = [
  { value: "spam", label: "Spam" },
  { value: "harassment", label: "Harassment or threats" },
  { value: "scam", label: "A scam" },
  { value: "hate", label: "Hate" },
  { value: "sexual", label: "Sexual content" },
  { value: "under13", label: "Someone under 13" },
  { value: "youngPersonRisk", label: "A risk to a young person" },
  { value: "impersonation", label: "Pretending to be someone else" },
  { value: "cheating", label: "Cheating for points" },
  { value: "other", label: "Something else" },
];

/** Said once in the Report window (ADR-167 §6): what a report carries. */
export const REPORT_EXPLAINED =
  "A report carries only what you tick, with proof it's real. 8 West reads only what you report.";

/** Said when a report was sent (ADR-167). */
export const REPORT_THANKS = "Thanks. 8 West will look at this.";

/** **Block**'s question, after its title ("Block @pat?"), in a system's own words (ADR-167 §1). */
export function blockWords(): string {
  return `They can't message you, find your card, or link with you. They aren't told. What is on ${systemWords().thisComputer} stays.`;
}

/** "Block @pat?": the title of the question. `name` is a Community name, without the "@". */
export function blockTitle(name: string): string {
  return `Block ${handle(name)}?`;
}

/** Said after a block that has no **Unblock** beside it: where to take it back. */
export const UNBLOCK_IN_SETTINGS = "You can undo it in Settings → Community → Blocked.";

/** Said in a conversation after you blocked its person. */
export function blockedWords(name: string): string {
  return `You blocked ${handle(name)}.`;
}

/** What the button in Settings → Community says. */
export function deleteDataLabel(): string {
  return `Delete my Community data from ${systemWords().thisComputer}`;
}

/** The question asked before **Delete my Community data from this PC** (ADR-168 §3). */
export function deleteDataWords(): string {
  return `Delete every Community conversation and message on ${systemWords().thisComputer}? 8 West and the other people keep theirs. This can't be undone.`;
}

/** Said when it is done. */
export const DELETED = "Deleted.";
