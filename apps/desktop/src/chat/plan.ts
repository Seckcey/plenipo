/** Where the tasks in an agent's plan stand (ADR-200), in plain words. */
import type { HandoffView } from "@plenipo/types";

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
