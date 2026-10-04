/**
 * The live conversation for one of a few tasks (Phase 25, item 3.1): Watch shows the one whose
 * file is on screen, or else the team's task that said something last.
 */
import { useContext } from "react";

import { AgentsContext } from "../agents/context";
import { saidAt } from "../agents/store";
import { LiveConversation } from "./LiveConversation";

export function LiveForTasks({
  taskIds,
  preferred,
  who,
}: {
  taskIds: readonly string[];
  /** The task to show when it has something to show (the file on screen's). */
  preferred: { taskId: string; sessionId: string | null } | null;
  who: string;
}) {
  const agents = useContext(AgentsContext);
  if (!agents) return null;
  const { activity, turns } = agents.state;
  // When it last said something: the newest piece of words that are still coming.
  const latest = (taskId: string) => {
    const last = activity[taskId]?.at(-1);
    return last ? saidAt(last) : 0;
  };
  const candidates = [...new Set([...(preferred ? [preferred.taskId] : []), ...taskIds])];
  const taskId =
    preferred && (activity[preferred.taskId]?.length ?? 0) > 0
      ? preferred.taskId
      : candidates.filter((t) => latest(t) > 0).sort((a, b) => latest(b) - latest(a))[0];
  if (!taskId) return null;
  const sessionId =
    (preferred?.taskId === taskId ? preferred.sessionId : null) ??
    activity[taskId]?.[0]?.sessionId ??
    null;
  const turn = sessionId ? turns[sessionId]?.find((t) => t.taskId === taskId) : undefined;
  return (
    <LiveConversation
      taskId={taskId}
      sessionId={sessionId}
      startedAt={turn?.startedAt ?? null}
      running={turn?.running ?? true}
      who={who}
    />
  );
}
