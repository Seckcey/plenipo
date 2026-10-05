/**
 * Who an agent works with (B5, on the Workers page): the agents that asked it for work, and the
 * ones it asked, with how many tasks each, from the conversations Plenipo lists (a worker's
 * conversation for a task names the conversation that asked it). Plain functions.
 */
import type { AgentSession } from "@plenipo/types";

import { liaisonInfo } from "../agents/store";

export interface Talks {
  /** Positions that asked this one for work, and how many times. */
  askedBy: { positionId: string; tasks: number }[];
  /** Positions this one asked for work, and how many times. */
  asked: { positionId: string; tasks: number }[];
}

function counted(ids: (string | null)[], self: string): { positionId: string; tasks: number }[] {
  const tally = new Map<string, number>();
  for (const id of ids) {
    if (id && id !== self) tally.set(id, (tally.get(id) ?? 0) + 1);
  }
  return [...tally].map(([positionId, tasks]) => ({ positionId, tasks }));
}

export function talksOf(sessions: Record<string, AgentSession>, positionId: string): Talks {
  const all = Object.values(sessions);
  const own = all.filter((s) => liaisonInfo(s).positionId === positionId);
  const ownIds = new Set(own.map((s) => s.id));
  const positionOf = (id: string | null) =>
    id && sessions[id] ? liaisonInfo(sessions[id]).positionId : null;
  return {
    askedBy: counted(
      own
        .filter((s) => liaisonInfo(s).origin === "handoff")
        .map((s) => positionOf(liaisonInfo(s).parentSessionId)),
      positionId,
    ),
    asked: counted(
      all
        .filter((s) => {
          const info = liaisonInfo(s);
          return (
            info.origin === "handoff" &&
            info.parentSessionId !== null &&
            ownIds.has(info.parentSessionId)
          );
        })
        .map((s) => liaisonInfo(s).positionId),
      positionId,
    ),
  };
}
