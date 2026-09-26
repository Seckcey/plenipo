import type { ExecutionRecord } from "@plenipo/types";

import { formatTime, STATE_LABEL } from "../runtime/format";
import { useRuntime } from "../runtime/useRuntime";

interface Entry {
  key: string;
  at: number;
  text: string;
  tone: "neutral" | "ok" | "bad";
}

function entriesFor(record: ExecutionRecord): Entry[] {
  const entries: Entry[] = [
    {
      key: `${record.id}:start`,
      at: record.startedAt,
      text: `${record.label} started`,
      tone: "neutral",
    },
  ];
  if (record.state !== "starting" && record.state !== "running") {
    const tone = record.state === "succeeded" ? "ok" : "bad";
    const detail = record.detail ? ` — ${record.detail}` : "";
    entries.push({
      key: `${record.id}:end`,
      at: record.endedAt ?? record.startedAt,
      text: `${record.label}: ${STATE_LABEL[record.state]}${detail}`,
      tone,
    });
  }
  return entries;
}

/** Activity derived from execution history, so it is identical after a reload. */
export function ActivityView() {
  const { state } = useRuntime();
  const entries = state.order
    .flatMap((id) => {
      const record = state.executions[id];
      return record ? entriesFor(record) : [];
    })
    .sort((a, b) => b.at - a.at);

  return (
    <section className="view" aria-labelledby="activity-title">
      <h1 id="activity-title">Activity</h1>
      {entries.length === 0 ? (
        <p className="muted">No activity yet.</p>
      ) : (
        <ol className="activity">
          {entries.map((e) => (
            <li key={e.key} className={`activity__item activity__item--${e.tone}`}>
              <time>{formatTime(e.at)}</time>
              <span>{e.text}</span>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
