/**
 * A tile's one line while it works (Phase 25, item 3.1): "now: Running `npm test`". Its own small
 * part, so only it redraws as the worker types, not the whole map.
 */
import { useContext, type ReactNode } from "react";
import type { PositionInfo } from "@plenipo/types";

import { AgentsContext } from "../agents/context";
import { liveWork, nowWords } from "./words";

export function NowLine({
  p,
  className,
  otherwise,
}: {
  p: PositionInfo;
  className: string;
  /** What the line says while it isn't working. */
  otherwise: ReactNode;
}) {
  const agents = useContext(AgentsContext);
  const work = liveWork(p)[0];
  const words = work ? nowWords(agents?.state.activity[work.taskId] ?? []) : null;
  if (!words) return <span className={className}>{otherwise}</span>;
  return (
    <span className={`${className} topo-node__now`} title={words}>
      now: {words}
    </span>
  );
}
