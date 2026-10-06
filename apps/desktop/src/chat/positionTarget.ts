import type { PositionInfo } from "@plenipo/types";

import { canTakeObjective } from "../org/rules";
import type { ChatTarget } from "./tabs";

/**
 * A position's chat (ADR-200): a full-time agent you talk to, or an on-call position, which you
 * talk to directly too, in a chat of its own (ADR-208). `null` for a position with no chat.
 */
export function positionChatTarget(p: PositionInfo): ChatTarget | null {
  const onCall = p.active && p.staffing === "onDemand";
  return canTakeObjective(p) || onCall
    ? { positionId: p.id, sessionId: p.agent?.sessionId ?? null, title: p.title }
    : null;
}
