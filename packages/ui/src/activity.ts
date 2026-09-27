/**
 * Activity over time, for strips, sparklines, and timelines (ADR-030 §7).
 *
 * Downsampling: a series always has a fixed number of buckets (96 for 24 hours: 15 minutes
 * each). A longer range only makes each bucket wider; counts are added up, so nothing is
 * dropped. The Ledger does the same in SQL (`Ledger::activity`); `bucketActivity` is the
 * in-memory version for lists already on screen.
 */

export interface ActivityBucket {
  /** Everything recorded in the bucket. */
  events: number;
  /** Failures, refusals, and timeouts (a task blocked on handoff replies is not a problem). */
  problems: number;
  /** Requests waiting for approval. */
  waiting: number;
}

export interface ActivitySeries {
  /** Start of the first bucket (ms since 1970). */
  from: number;
  /** End of the last bucket (ms since 1970). */
  to: number;
  /** Width of each bucket (ms). */
  bucketMs: number;
  buckets: ActivityBucket[];
}

export type ActivityKind = "event" | "problem" | "waiting";

export const HOUR_MS = 3_600_000;
export const DAY_MS = 24 * HOUR_MS;
/** 24 hours in 15-minute buckets. */
export const DEFAULT_BUCKETS = 96;

const emptyBucket = (): ActivityBucket => ({ events: 0, problems: 0, waiting: 0 });

/** Count `items` into `count` equal buckets over [from, to). Items outside are ignored. */
export function bucketActivity(
  items: readonly { at: number; kind?: ActivityKind }[],
  from: number,
  to: number,
  count: number = DEFAULT_BUCKETS,
): ActivitySeries {
  const n = Math.max(1, Math.floor(count));
  const span = Math.max(1, to - from);
  const bucketMs = Math.ceil(span / n);
  const buckets = Array.from({ length: n }, emptyBucket);
  for (const item of items) {
    if (item.at < from || item.at >= to) continue;
    const b = buckets[Math.min(n - 1, Math.floor((item.at - from) / bucketMs))];
    if (!b) continue;
    b.events += 1;
    if (item.kind === "problem") b.problems += 1;
    if (item.kind === "waiting") b.waiting += 1;
  }
  return { from, to, bucketMs, buckets };
}

/**
 * Merge a series into at most `count` buckets by adding equal runs of neighbours (never drops a
 * count). Each merged bucket is `bucketMs` wide; the last may cover fewer original buckets.
 */
export function downsample(series: ActivitySeries, count: number): ActivitySeries {
  const n = Math.max(1, Math.floor(count));
  if (series.buckets.length <= n) return series;
  const size = Math.ceil(series.buckets.length / n);
  const buckets: ActivityBucket[] = [];
  for (let i = 0; i < series.buckets.length; i += size) {
    const merged = emptyBucket();
    for (const b of series.buckets.slice(i, i + size)) {
      merged.events += b.events;
      merged.problems += b.problems;
      merged.waiting += b.waiting;
    }
    buckets.push(merged);
  }
  return { ...series, bucketMs: series.bucketMs * size, buckets };
}

export interface ActivityTotals {
  events: number;
  problems: number;
  waiting: number;
}

export function totals(series: ActivitySeries): ActivityTotals {
  return series.buckets.reduce(
    (t, b) => ({
      events: t.events + b.events,
      problems: t.problems + b.problems,
      waiting: t.waiting + b.waiting,
    }),
    { events: 0, problems: 0, waiting: 0 },
  );
}

export type BucketLevel = "none" | "low" | "mid" | "high" | "waiting" | "problem";

/**
 * How a bucket is drawn: problems first, then waiting, then how busy it was compared with the
 * busiest bucket (three steps, so one burst does not wash the rest out).
 */
export function bucketLevel(b: ActivityBucket, max: number): BucketLevel {
  if (b.problems > 0) return "problem";
  if (b.waiting > 0) return "waiting";
  if (b.events === 0) return "none";
  const share = max > 0 ? b.events / max : 1;
  if (share > 0.66) return "high";
  if (share > 0.33) return "mid";
  return "low";
}

const timeFormat = new Intl.DateTimeFormat("en-US", { hour: "numeric", minute: "2-digit" });

/** "3:15 PM" in this computer's time zone. */
export const formatTime = (ms: number) => timeFormat.format(new Date(ms));

const dayFormat = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" });

/**
 * Time axis under a strip: start, middle, and "Now" when the series ends at the present. Times
 * of day for a day or less; dates ("Sep 20") for longer strips.
 */
export function axisLabels(
  series: Pick<ActivitySeries, "from" | "to">,
  now: number = Date.now(),
): [string, string, string] {
  const endsNow = Math.abs(now - series.to) <= Math.max(60_000, (series.to - series.from) / 96);
  const label =
    series.to - series.from >= 2 * DAY_MS
      ? (ms: number) => dayFormat.format(new Date(ms))
      : formatTime;
  return [
    label(series.from),
    label(series.from + (series.to - series.from) / 2),
    endsNow ? "Now" : label(series.to),
  ];
}

/** "24 hours", "7 days", "90 minutes". */
export function describeRange(ms: number): string {
  if (ms >= 2 * DAY_MS && ms % DAY_MS === 0) return `${ms / DAY_MS} days`;
  if (ms >= 2 * HOUR_MS && ms % HOUR_MS === 0) return `${ms / HOUR_MS} hours`;
  if (ms === HOUR_MS) return "1 hour";
  return `${Math.round(ms / 60_000)} minutes`;
}

/** Spoken summary of a strip. */
export function describeActivity(series: ActivitySeries): string {
  const t = totals(series);
  const parts = [`${t.events} ${t.events === 1 ? "event" : "events"}`];
  if (t.problems) parts.push(`${t.problems} ${t.problems === 1 ? "problem" : "problems"}`);
  if (t.waiting) parts.push(`${t.waiting} waiting for approval`);
  return `Last ${describeRange(series.to - series.from)}: ${parts.join(", ")}`;
}

/** Reduce `values` to at most `count` points by averaging neighbours (sparklines). */
export function averageDown(values: readonly number[], count: number): number[] {
  const n = Math.max(1, Math.floor(count));
  if (values.length <= n) return [...values];
  const size = values.length / n;
  const out: number[] = [];
  for (let i = 0; i < n; i++) {
    // The last point always ends at the newest value (no rounding can drop it).
    const end = i === n - 1 ? values.length : Math.floor((i + 1) * size);
    const part = values.slice(Math.floor(i * size), end);
    out.push(part.reduce((a, b) => a + b, 0) / Math.max(1, part.length));
  }
  return out;
}
