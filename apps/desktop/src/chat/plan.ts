/** Where the tasks in an agent's plan stand (ADR-200), in plain words. */
import type { ChainOrder, HandoffView } from "@plenipo/types";

/** Where a task stands, as the circle beside it shows (always with words, never color alone). */
export type Standing = "waiting" | "working" | "done" | "failed" | "stopped";

export const STANDING_WORDS: Record<Standing, string> = {
  waiting: "Waiting",
  working: "Working",
  done: "Done",
  failed: "Did not finish",
  stopped: "Stopped",
};

/** Where a hand-off stands: from its worker's task first, then from the hand-off itself. */
export function standingOf(h: HandoffView): Standing {
  if (h.state === "rejected") return "failed";
  if (h.state === "cancelled") return "stopped";
  switch (h.childState) {
    case "running":
      return "working";
    case "succeeded":
      return "done";
    case "failed":
      return "failed";
    case "cancelled":
      return "stopped";
    default:
      return h.state === "answered" ? "done" : "waiting";
  }
}

/** The first line of a task, short. */
export function firstLine(text: string, longest = 140): string {
  const line = (text.split("\n").find((l) => l.trim() !== "") ?? "").trim();
  return line.length > longest ? `${line.slice(0, longest - 1)}…` : line;
}

/** "A", "A and B", "A, B, and C". */
export function listWords(names: readonly string[]): string {
  if (names.length <= 2) return names.join(" and ");
  return `${names.slice(0, -1).join(", ")}, and ${names[names.length - 1] ?? ""}`;
}

/** One of your orders in a position's chain of command, in plain words (ADR-202). */
export function chainLine(o: ChainOrder): string {
  const words = `“${firstLine(o.words, 100)}”`;
  switch (o.part) {
    case "doer":
      if (o.via) return `You asked it, through ${o.via}: ${words}.`;
      return o.leads.length > 0
        ? `You asked it directly: ${words}. Plenipo told ${listWords(o.leads)}.`
        : `You asked it: ${words}.`;
    case "via":
      return `You asked its ${o.position}, through it: ${words}.`;
    case "told":
      return `You asked ${o.position} ${o.via ? `through ${o.via}` : "directly"}: ${words}.`;
  }
}
