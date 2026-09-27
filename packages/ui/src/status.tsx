/**
 * Status primitives. Status is always a mark **and** a word, never color alone (ADR-030 §5):
 * each status also has its own mark shape, so it reads in grayscale too.
 */

import type { ReactNode } from "react";

import {
  axisLabels,
  averageDown,
  bucketLevel,
  describeActivity,
  totals,
  type ActivitySeries,
} from "./activity";
import { cx, formatCount } from "./util";

import type { Status } from "./status-types";

export type { Status } from "./status-types";

/** A small status mark with its word. */
export function StatusDot({
  status,
  label,
  className,
}: {
  status: Status;
  /** The word, e.g. "Working". Always shown. */
  label: string;
  className?: string;
}) {
  return (
    <span className={cx("ui-status", `ui-status--${status}`, className)} data-status={status}>
      <span className="ui-status__mark" aria-hidden="true" />
      <span className="ui-status__label">{label}</span>
    </span>
  );
}

/** A status word in a tinted pill, for headers and cards. */
export function StatusPill({
  status,
  label,
  className,
}: {
  status: Status;
  label: string;
  className?: string;
}) {
  return (
    <span className={cx("ui-pill", `ui-pill--${status}`, className)} data-status={status}>
      <span className="ui-status__mark" aria-hidden="true" />
      {label}
    </span>
  );
}

/**
 * A plain tag: a name, not a state ("Run programs", "Handoff worker"). No mark, because a status
 * mark always means a state.
 */
export function Tag({ label, className }: { label: string; className?: string | undefined }) {
  return <span className={cx("ui-tag", className)}>{label}</span>;
}

/** A number in a small badge ("3"), with its meaning spoken ("3 waiting for you"). */
export function CountBadge({
  count,
  label,
  tone = "accent",
  showZero = false,
  className,
}: {
  count: number;
  /** Spoken meaning; the number is prefixed: `${count} ${label}`. */
  label: string;
  tone?: "accent" | Status;
  showZero?: boolean;
  className?: string;
}) {
  if (count === 0 && !showZero) return null;
  return (
    <span
      className={cx("ui-badge", `ui-badge--${tone}`, className)}
      role="img"
      aria-label={`${count} ${label}`}
    >
      {count > 999 ? "999+" : count}
    </span>
  );
}

/** A value against a maximum, e.g. "7 of 10 done", with its words next to the bar. */
export function HealthBar({
  value,
  max,
  label,
  valueText,
  status = "ok",
}: {
  value: number;
  max: number;
  label: string;
  /** Shown and spoken, e.g. "7 of 10". Defaults to a percentage. */
  valueText?: string;
  status?: Status;
}) {
  const share = max > 0 ? Math.max(0, Math.min(1, value / max)) : 0;
  const text = valueText ?? `${Math.round(share * 100)}%`;
  return (
    <div className="ui-health">
      <div className="ui-health__caption">
        <span>{label}</span>
        <span className="ui-num">{text}</span>
      </div>
      <div
        className={cx("ui-health__track", `ui-health--${status}`)}
        role="meter"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={max}
        aria-valuenow={value}
        aria-valuetext={text}
      >
        <span className="ui-health__fill" style={{ width: `${share * 100}%` }} />
      </div>
    </div>
  );
}

/** A tiny line chart of recent values. */
export function Sparkline({
  values,
  label,
  width = 96,
  height = 24,
  status,
}: {
  values: readonly number[];
  /** Spoken, with the range, e.g. "Events per hour". */
  label: string;
  width?: number;
  height?: number;
  status?: Status;
}) {
  const points = averageDown(values, width / 2);
  if (points.length === 0) {
    return (
      <span className="ui-spark ui-spark--empty" role="img" aria-label={`${label}: no data yet`}>
        No data yet
      </span>
    );
  }
  const max = Math.max(...points, 1);
  const min = Math.min(...points, 0);
  const step = points.length > 1 ? width / (points.length - 1) : width;
  const y = (v: number) => height - 2 - ((v - min) / (max - min || 1)) * (height - 4);
  const d = points.map((v, i) => `${i ? "L" : "M"}${(i * step).toFixed(1)},${y(v).toFixed(1)}`);
  const last = points[points.length - 1] ?? 0;
  return (
    <svg
      className={cx("ui-spark", status && `ui-spark--${status}`)}
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      role="img"
      aria-label={`${label}: lowest ${formatCount(Math.round(Math.min(...values)))}, highest ${formatCount(Math.round(Math.max(...values)))}, latest ${formatCount(Math.round(values[values.length - 1] ?? 0))}`}
    >
      <path d={d.join("")} className="ui-spark__line" />
      <circle cx={(points.length - 1) * step} cy={y(last)} r={2} className="ui-spark__dot" />
    </svg>
  );
}

/**
 * The 24-hour activity strip (the Site Manager card's strip): one segment per bucket, a time
 * axis with a "Now" marker, and a spoken summary.
 */
export function ActivityStrip({
  series,
  now,
  label = "Activity",
  axis = true,
  className,
}: {
  series: ActivitySeries;
  now?: number | undefined;
  label?: string;
  axis?: boolean;
  className?: string;
}) {
  const max = Math.max(0, ...series.buckets.map((b) => b.events));
  const [start, middle, end] = axisLabels(series, now);
  const t = totals(series);
  return (
    <div className={cx("ui-strip", className)}>
      <div
        className="ui-strip__bar"
        role="img"
        aria-label={`${label}. ${describeActivity(series)}`}
        data-events={t.events}
      >
        {series.buckets.map((b, i) => (
          <span key={i} className={`ui-strip__seg ui-strip__seg--${bucketLevel(b, max)}`} />
        ))}
        {end === "Now" && <span className="ui-strip__now" aria-hidden="true" />}
      </div>
      {axis && (
        <div className="ui-strip__axis ui-num" aria-hidden="true">
          <span>{start}</span>
          <span>{middle}</span>
          <span className={end === "Now" ? "ui-strip__axis-now" : undefined}>{end}</span>
        </div>
      )}
    </div>
  );
}

/** The strip's place while its data loads, or when it could not be read. */
export function ActivityStripPlaceholder({
  state,
  message,
}: {
  state: "loading" | "error" | "empty";
  message?: ReactNode;
}) {
  return (
    <div className={cx("ui-strip", `ui-strip--${state}`)}>
      <div className="ui-strip__bar" aria-hidden="true" />
      <div className="ui-strip__message">
        {message ??
          (state === "loading"
            ? "Loading activity…"
            : state === "error"
              ? "Couldn't load activity"
              : "No activity yet")}
      </div>
    </div>
  );
}
