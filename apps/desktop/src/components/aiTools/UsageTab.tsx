import type { AiToolUsage } from "@plenipo/types";
import { LoadingState } from "@plenipo/ui";

import { tokens } from "../../routing/format";
import {
  cachingLine,
  countsNothing,
  dayName,
  DEFAULT_MODEL,
  usageBetween,
  usageLine,
  usageModels,
  type UsageWindow,
} from "./words";

/**
 * A card's Usage (ADR-060 §1): tokens (pieces of words) read and written, by model, today, this
 * week, and last week (weeks run Monday to Sunday), and each of the last 14 days. Added up from
 * what each task's steps recorded.
 */
export function UsageTab({
  label,
  usage,
}: {
  label: string;
  usage: { usage: AiToolUsage | null; window: UsageWindow | null; error: string | null };
}) {
  const u = usage.usage;
  const w = usage.window;
  if (!u || !w) {
    return usage.error ? (
      <p className="form-error">
        Couldn&apos;t read {label}&apos;s usage: {usage.error}
      </p>
    ) : (
      <LoadingState label={`Loading ${label}'s usage`} lines={2} />
    );
  }
  const counts = !countsNothing(u);
  const models = usageModels(u);
  const periods: [string, number, number][] = [
    ["Today", w.today, w.tomorrow],
    ["This week", w.thisWeek, w.tomorrow],
    ["Last week", w.lastWeek, w.thisWeek],
  ];
  const sums = periods.map(([, from, to]) => usageBetween(u, from, to));
  const days = w.starts.filter((d) => d >= w.last14 && d < w.tomorrow).reverse();
  const dayTotal = (start: number) => usageBetween(u, start, start + 1).total;

  return (
    <div className="ai-tool__usage">
      <p className="muted">
        Tokens are pieces of words: what the AI tool read and what it wrote. Reused tokens were read
        again from earlier in the conversation, which usually costs less. Weeks run Monday to
        Sunday. These count the tasks Plenipo ran on {label}, in every organization; your own use of{" "}
        {label} outside Plenipo isn&apos;t counted.
      </p>
      {!counts && (
        <p role="note">{label} doesn&apos;t report token counts, so only its tasks are counted.</p>
      )}
      <h3>In all</h3>
      <ul className="ai-tool__list" aria-label={`${label}'s usage in all`}>
        {periods.map(([name], i) => {
          const total = sums[i]?.total;
          return (
            <li key={name}>
              <strong>{name}:</strong>{" "}
              {total && total.tasks > 0 ? usageLine(total, counts) : "no tasks"}
            </li>
          );
        })}
      </ul>
      {counts && (
        <>
          <h3>Saved by caching</h3>
          <p className="muted">
            Read from the cache instead of fresh: on a paid key it costs about a tenth as much, and
            it uses less of a plan.
          </p>
          <ul className="ai-tool__list" aria-label={`What caching saved on ${label}`}>
            {periods.map(([name], i) => {
              const total = sums[i]?.total;
              return (
                <li key={name}>
                  <strong>{name}:</strong> {(total && cachingLine(total)) ?? "nothing reused"}
                </li>
              );
            })}
          </ul>
        </>
      )}
      <h3>By model</h3>
      {models.length === 0 ? (
        <p className="muted">No tasks on {label} in the last two weeks.</p>
      ) : (
        <div className="ai-tool__table">
          <table className="table" aria-label={`${label}'s usage by model`}>
            <thead>
              <tr>
                <th scope="col">Model</th>
                {periods.map(([name]) => (
                  <th key={name} scope="col">
                    {name}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {models.map((m) => (
                <tr key={m}>
                  <th scope="row">{m === DEFAULT_MODEL ? "The AI tool's default" : m}</th>
                  {sums.map(({ byModel }, i) => {
                    const sum = byModel.get(m);
                    return (
                      <td key={periods[i]?.[0]}>
                        {sum && sum.tasks > 0 ? usageLine(sum, counts) : "—"}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <h3>The last 14 days</h3>
      <div className="ai-tool__table">
        <table className="table" aria-label={`${label}'s usage in the last 14 days`}>
          <thead>
            <tr>
              <th scope="col">Day</th>
              {counts && (
                <>
                  <th scope="col">Read</th>
                  <th scope="col">Reused</th>
                  <th scope="col">Written</th>
                </>
              )}
              <th scope="col">Tasks</th>
            </tr>
          </thead>
          <tbody>
            {days.map((d) => {
              const t = dayTotal(d);
              return (
                <tr key={d}>
                  <th scope="row">{dayName(d, w.today)}</th>
                  {counts && (
                    <>
                      <td>{tokens(t.read)}</td>
                      <td>{tokens(t.reused)}</td>
                      <td>{tokens(t.written)}</td>
                    </>
                  )}
                  <td>{t.tasks}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
