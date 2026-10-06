import type { TaskCostPart } from "@plenipo/types";

import { useAgentsIfAny } from "../agents/useAgents";
import { moneyAmount, moneyWords, pricedWords, tokensDetail, tokensWords } from "./costWords";
import { useTaskTreeCost } from "./useTaskCost";

/**
 * What an objective cost (I2): its tokens across every worker that worked on it (the task and
 * every task handed out beneath it) and, on paid keys, its money; one line per worker. `compact`
 * is the one line alone (an objective's result card). Kept current as the Ledger records more.
 */
export function TaskCostSummary({
  taskId,
  compact = false,
}: {
  taskId: string;
  compact?: boolean;
}) {
  const tree = useTaskTreeCost(taskId);
  const agents = useAgentsIfAny();
  if (!tree) return null;
  const total = tree.total;
  if (total.runs === 0) {
    return compact ? null : <p className="muted">No AI tool has worked on it yet.</p>;
  }
  const workers = tree.parts.filter((p) => p.cost.runs > 0);
  const money = moneyAmount(total);
  const line = [
    tokensWords(total) ?? "no tokens reported",
    workers.length > 1 ? `across ${workers.length} workers` : null,
    money ? `${money} on paid keys` : null,
  ]
    .filter(Boolean)
    .join(" · ");
  // Said once, at the end: while any of it runs or money is still set aside.
  const sofar = total.running || total.setAsideMicros > 0;
  const who = (part: TaskCostPart) =>
    part.positionTitle ??
    agents?.state.runtimes.find((r) => r.id === part.runtime)?.label ??
    part.runtime ??
    "A worker";
  const priced = pricedWords(total);
  return (
    <div className="task-cost" aria-label="Tokens and cost">
      <p className={compact ? "task-cost__line muted" : "task-cost__line"}>
        Used {line}
        {sofar && " so far"}
      </p>
      {!compact && (
        <>
          {total.counted > 0 && <p className="muted">{tokensDetail(total)}</p>}
          {priced && <p className="muted">{priced}</p>}
          {workers.length > 1 && (
            <ul className="task-cost__parts" aria-label="By worker">
              {workers.map((part) => (
                <li key={part.taskId}>
                  {who(part)} · {tokensWords(part.cost) ?? "no tokens reported"}
                  {moneyWords(part.cost) && ` · ${moneyWords(part.cost)}`}
                </li>
              ))}
            </ul>
          )}
          {tree.more && (
            <p className="muted">It has more than 200 tasks: only the first 200 are counted.</p>
          )}
        </>
      )}
    </div>
  );
}
