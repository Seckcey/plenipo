import type { PositionInfo } from "@plenipo/types";

/** Whether a position can be asked a question (Phase 25, item 3.5): a full-time agent, hired. */
export const canAsk = (p: PositionInfo) => p.active && p.staffing === "persistent" && !!p.agent;
