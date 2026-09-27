import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { LedgerEvent } from "@plenipo/types";
import { Button, EmptyState, RowList, type RowItem } from "@plenipo/ui";

import { toCommandError } from "../api/commands";
import { shownInTrail } from "../ledger/format";
import { ago } from "../org/format";
import { anyEvent, useLive } from "./useLive";
import { eventStatus, historyLine } from "./words";

/** Events per page of history. */
export const HISTORY_PAGE = 50;

/**
 * A history of events, newest first, kept live: the newest page reloads as things happen, and
 * **Show older** adds the page before the oldest shown. `load(before)` reads the page before
 * the event numbered `before` (all of it when `before` is left out). Once older pages are shown,
 * the events between them and a newest page that moved on are read too, so none go missing.
 */
export function EventHistory({
  label,
  historyKey,
  load,
  now,
  onOpenTask,
  currentTaskId = null,
  empty = "Nothing has happened here yet.",
}: {
  label: string;
  /** Which history this is (a new key starts again from the newest page). */
  historyKey: string;
  load: (before?: number) => Promise<LedgerEvent[]>;
  now: number;
  /** Opens the task an event belongs to. */
  onOpenTask?: ((taskId: string) => void) | undefined;
  /** The task whose page this is (its own events don't open it again). */
  currentTaskId?: string | null;
  empty?: string;
}) {
  const newest = useLive(historyKey, () => load(), anyEvent, 1_000);
  // Older pages, kept with the history they belong to.
  const [older, setOlder] = useState<{ key: string; events: LedgerEvent[]; done: boolean }>({
    key: "",
    events: [],
    done: false,
  });
  const [paging, setPaging] = useState<{ key: string; error: string | null; busy: boolean }>({
    key: "",
    error: null,
    busy: false,
  });
  const olderEvents = older.key === historyKey ? older.events : [];
  const page = paging.key === historyKey ? paging : { error: null, busy: false };
  // The newest `load` (callers pass a new function on every render).
  const loadRef = useRef(load);
  useLayoutEffect(() => {
    loadRef.current = load;
  });

  // When many events arrive at once, the newest page moves past the events just above the
  // older pages shown. Read those in between (a page at a time, until the older pages are
  // reached), so the history stays whole.
  const filling = useRef(false);
  useEffect(() => {
    const top = newest.value;
    if (older.key !== historyKey || older.events.length === 0 || !top?.length) return;
    if (filling.current) return;
    const key = historyKey;
    const reached = older.events.reduce((max, e) => Math.max(max, e.seq), 0);
    let before = top.reduce((min, e) => Math.min(min, e.seq), Number.MAX_SAFE_INTEGER);
    if (before <= reached) return;
    const known = new Set([...top, ...older.events].map((e) => e.seq));
    filling.current = true;
    void (async () => {
      const found: LedgerEvent[] = [];
      try {
        for (let round = 0; round < 20; round++) {
          const between = await loadRef.current(before);
          found.push(...between.filter((e) => !known.has(e.seq)));
          const last = between.at(-1);
          if (!last || between.length < HISTORY_PAGE || last.seq <= reached) break;
          before = last.seq;
        }
      } catch {
        // The next reload tries again.
      } finally {
        filling.current = false;
      }
      if (found.length > 0) {
        setOlder((prev) =>
          prev.key === key ? { ...prev, events: [...prev.events, ...found] } : prev,
        );
      }
    })();
  }, [newest.value, older, historyKey]);

  const seen = new Set<number>();
  const events = [...(newest.value ?? []), ...olderEvents]
    .filter((e) => {
      if (seen.has(e.seq)) return false;
      seen.add(e.seq);
      return shownInTrail(e);
    })
    .sort((a, b) => b.seq - a.seq);
  const oldest = [...(newest.value ?? []), ...olderEvents].reduce<number | null>(
    (min, e) => (min === null || e.seq < min ? e.seq : min),
    null,
  );
  const done =
    (older.key === historyKey && older.done) ||
    (older.key !== historyKey && (newest.value?.length ?? 0) < HISTORY_PAGE);

  const more = async () => {
    if (oldest === null) return;
    const key = historyKey;
    setPaging({ key, error: null, busy: true });
    try {
      const next = await load(oldest);
      setOlder((prev) => ({
        key,
        events: [...(prev.key === key ? prev.events : []), ...next],
        done: next.length < HISTORY_PAGE,
      }));
      setPaging({ key, error: null, busy: false });
    } catch (reason) {
      setPaging({ key, error: toCommandError(reason).message, busy: false });
    }
  };

  const items: RowItem[] = events.map((e) => {
    const opens = e.taskId !== null && e.taskId !== currentTaskId && onOpenTask;
    return {
      id: String(e.seq),
      title: historyLine(e),
      status: eventStatus(e),
      meta: ago(e.createdAt, now),
      onOpen: opens ? () => onOpenTask(e.taskId!) : undefined,
      openLabel: opens ? `${historyLine(e)}. Open its task` : undefined,
    };
  });

  return (
    <div className="page__history">
      <RowList
        label={label}
        items={items}
        state={newest.status}
        error={newest.error}
        onRetry={newest.reload}
        empty={<EmptyState compact title={empty} />}
      />
      {newest.status === "ready" && events.length > 0 && !done && (
        <Button size="sm" variant="quiet" onClick={() => void more()} disabled={page.busy}>
          {page.busy ? "Loading…" : "Show older"}
        </Button>
      )}
      {page.error && (
        <p className="page__note page__note--error" role="alert">
          Couldn't load older events: {page.error}
        </p>
      )}
    </div>
  );
}
