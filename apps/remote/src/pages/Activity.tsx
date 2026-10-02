import { useState } from "react";
import type { LedgerEvent } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { useRead, useSession } from "../session-context";
import { ago, describe } from "../words";

/**
 * Activity: what the PC recorded, newest first, including what each phone asked. What the phone
 * has no plain words for stays in Activity on the PC, under All events.
 */
export function ActivityPage() {
  const { org, ask } = useSession();
  const { data, error, reload } = useRead<LedgerEvent[]>({ kind: "readActivity", org }, [
    "approvals",
    "tasks",
    "control",
    "lessons",
  ]);
  const [older, setOlder] = useState<LedgerEvent[]>([]);
  const [more, setMore] = useState(true);
  if (error) {
    return (
      <section className="page">
        <h1>Activity</h1>
        <p className="form-error" role="alert">
          {error}
        </p>
        <Button onClick={reload}>Try again</Button>
      </section>
    );
  }
  if (!data) return <p role="status">Loading Activity…</p>;
  const all = [...data, ...older];
  const last = all[all.length - 1];
  const shown = all.flatMap((e) => {
    const words = describe(e);
    return words ? [{ e, words }] : [];
  });
  return (
    <section className="page" aria-labelledby="activity-title">
      <h1 id="activity-title">Activity</h1>
      {shown.length === 0 ? (
        <p className="muted">Nothing to show here yet. Activity on your PC has everything.</p>
      ) : (
        <ul className="list">
          {shown.map(({ e, words }) => (
            <li key={e.id} className="card card--quiet">
              <span>{words}</span>
              <p className="muted">
                {ago(e.createdAt)}
                {e.source === "owner" ? " · you" : ""}
              </p>
            </li>
          ))}
        </ul>
      )}
      {more && last && data.length >= 50 && (
        <Button
          onClick={() => {
            ask({ kind: "readActivity", org, before: last.seq }, true)
              .then((r) => {
                const page = (r.ok as LedgerEvent[] | undefined) ?? [];
                setOlder((o) => [...o, ...page]);
                if (page.length < 50) setMore(false);
              })
              .catch(() => undefined);
          }}
        >
          Older
        </Button>
      )}
    </section>
  );
}
