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

/**
 * Pair each request the lead wrote with Liaison's record of it, in order: by its words and the
 * worker it went to first (two requests with the same words to different workers keep their own
 * cards), then by its words alone for any left over.
 */
export function matchRequests(
  asked: readonly { to: string; objective: string }[],
  sent: readonly HandoffView[],
): (HandoffView | null)[] {
  const left: (HandoffView | null)[] = [...sent];
  const take = (match: (v: HandoffView) => boolean): HandoffView | null => {
    const i = left.findIndex((v) => v !== null && match(v));
    if (i < 0) return null;
    const view = left[i] ?? null;
    left[i] = null;
    return view;
  };
  const same = (v: HandoffView, a: { objective: string }) =>
    v.objective.trim() === a.objective.trim();
  const exact = asked.map((a) => take((v) => same(v, a) && v.destination.trim() === a.to.trim()));
  return asked.map((a, i) => exact[i] ?? take((v) => same(v, a)));
}

/** The conversation a request came from, or `null`. */
export function sessionOfSource(source: string): string | null {
  return source.startsWith("session:") ? source.slice("session:".length) || null : null;
}
