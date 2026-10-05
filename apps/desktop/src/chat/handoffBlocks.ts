/**
 * A lead asks another worker for help by ending its answer with a fenced `plenipo-handoff` block
 * (the Liaison protocol, ADR-008). The chat shows each one as a card ("Handed to …", B5) instead
 * of its raw words. Only a closed block is a request; one still being written stays words until
 * it is closed, and a block that is not a request (not JSON, no `to`) stays as it was written.
 */

import type { HandoffView } from "@plenipo/types";

export type Piece =
  { kind: "text"; text: string } | { kind: "handoff"; to: string; objective: string };

const FENCE = /^```plenipo-handoff[^\n]*\n([\s\S]*?)\n```[ \t]*$/gm;

function request(body: string): { to: string; objective: string } | null {
  try {
    const value: unknown = JSON.parse(body);
    if (!value || typeof value !== "object") return null;
    const { to, objective } = value as Record<string, unknown>;
    if (typeof to !== "string" || to.trim() === "") return null;
    return { to: to.trim(), objective: typeof objective === "string" ? objective.trim() : "" };
  } catch {
    return null;
  }
}

/** A lead's words, with each handoff request it wrote taken out as a request of its own. */
export function splitHandoffs(text: string): Piece[] {
  const pieces: Piece[] = [];
  let last = 0;
  for (const match of text.matchAll(FENCE)) {
    const asked = request(match[1] ?? "");
    if (!asked) continue;
    const before = text.slice(last, match.index);
    if (before.trim() !== "") pieces.push({ kind: "text", text: before });
    pieces.push({ kind: "handoff", ...asked });
    last = match.index + match[0].length;
  }
  const rest = text.slice(last);
  if (rest.trim() !== "" || pieces.length === 0) pieces.push({ kind: "text", text: rest });
  return pieces;
}

/** Pair each request the lead wrote with Liaison's record of it, by its words, in order. */
export function matchRequests(
  asked: readonly { objective: string }[],
  sent: readonly HandoffView[],
): (HandoffView | null)[] {
  const left = [...sent];
  return asked.map((a) => {
    const i = left.findIndex((v) => v.objective.trim() === a.objective.trim());
    if (i < 0) return null;
    const [view] = left.splice(i, 1);
    return view ?? null;
  });
}

/** The conversation a request came from, or `null`. */
export function sessionOfSource(source: string): string | null {
  return source.startsWith("session:") ? source.slice("session:".length) || null : null;
}
