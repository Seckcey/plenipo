/**
 * The detail split view (the UniFi port view): properties and switches on the left, a live
 * timeline in the middle, a map on the right, and a table below. Plus its parts: the property
 * list and the timeline scrubber.
 */

import { useRef, type KeyboardEvent, type ReactNode } from "react";

import { formatTime, HOUR_MS } from "./activity";
import { Icon } from "./icons";
import { cx } from "./util";
import type { Status } from "./status";

export function DetailSplitView({
  label,
  properties,
  timeline,
  map,
  table,
}: {
  label: string;
  properties: ReactNode;
  timeline?: ReactNode;
  map?: ReactNode;
  table?: ReactNode;
}) {
  return (
    <section className="ui-split" aria-label={label}>
      <div className="ui-split__properties">{properties}</div>
      {timeline && <div className="ui-split__timeline">{timeline}</div>}
      {map && <div className="ui-split__map">{map}</div>}
      {table && <div className="ui-split__table">{table}</div>}
    </section>
  );
}

/** Label/value rows, with an optional heading and switches. */
export function PropertyList({
  title,
  items,
}: {
  title?: ReactNode;
  items: readonly { label: string; value: ReactNode }[];
}) {
  return (
    <div className="ui-props">
      {title && <div className="ui-props__title">{title}</div>}
      <dl>
        {items.map((item) => (
          <div key={item.label} className="ui-props__row">
            <dt>{item.label}</dt>
            <dd>{item.value}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

export interface TimelineEvent {
  at: number;
  label: string;
  status?: Status;
}

/** `"live"` follows the present; a number is a moment in the past. */
export type TimelineValue = number | "live";

const STEP = 15 * 60_000;

/**
 * A vertical timeline scrubber: the present at the top, "Live" when following it. Drag or
 * click the track, or use the arrow keys (15 minutes), Page Up/Down (1 hour), Home (oldest),
 * and End (back to live).
 */
export function TimelineScrubber({
  from,
  to,
  events,
  value,
  onChange,
  label = "Timeline",
}: {
  from: number;
  to: number;
  events: readonly TimelineEvent[];
  value: TimelineValue;
  onChange: (next: TimelineValue) => void;
  label?: string;
}) {
  const track = useRef<HTMLDivElement>(null);
  const span = Math.max(1, to - from);
  const at = value === "live" ? to : Math.min(to, Math.max(from, value));
  /** 0 at the top (now), 1 at the bottom (oldest). */
  const pos = (t: number) => (to - t) / span;
  const set = (t: number) =>
    onChange(t >= to - STEP / 2 ? "live" : Math.max(from, Math.min(to, t)));

  const onKey = (e: KeyboardEvent) => {
    const moves: Record<string, () => void> = {
      ArrowUp: () => set(at + STEP),
      ArrowRight: () => set(at + STEP),
      ArrowDown: () => set(at - STEP),
      ArrowLeft: () => set(at - STEP),
      PageUp: () => set(at + HOUR_MS),
      PageDown: () => set(at - HOUR_MS),
      Home: () => set(from),
      End: () => onChange("live"),
    };
    const move = moves[e.key];
    if (!move) return;
    e.preventDefault();
    move();
  };

  const pick = (clientY: number) => {
    const rect = track.current?.getBoundingClientRect();
    if (!rect || rect.height === 0) return;
    const share = Math.max(0, Math.min(1, (clientY - rect.top) / rect.height));
    set(to - share * span);
  };

  const ticks = Array.from({ length: 7 }, (_, i) => to - (span * i) / 6);
  const hours = Math.round(span / HOUR_MS);
  const inRange = events.filter((e) => e.at >= from && e.at <= to);

  return (
    <div className="ui-timeline">
      <div className="ui-timeline__head">
        Last {hours} {hours === 1 ? "hour" : "hours"}{" "}
        <span className="ui-num">
          ({inRange.length} {inRange.length === 1 ? "event" : "events"})
        </span>
      </div>
      <div
        className="ui-timeline__track"
        ref={track}
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture?.(e.pointerId);
          pick(e.clientY);
        }}
        onPointerMove={(e) => {
          if (e.buttons === 1) pick(e.clientY);
        }}
      >
        <span className="ui-timeline__rail" aria-hidden="true" />
        {ticks.map((t) => (
          <span
            key={t}
            className="ui-timeline__tick ui-num"
            style={{ top: `${pos(t) * 100}%` }}
            aria-hidden="true"
          >
            {formatTime(t)}
          </span>
        ))}
        {inRange.map((e, i) => (
          <span
            key={`${e.at}-${i}`}
            className={cx("ui-timeline__event", e.status && `ui-timeline__event--${e.status}`)}
            style={{ top: `${pos(e.at) * 100}%` }}
            title={`${formatTime(e.at)} · ${e.label}`}
            aria-hidden="true"
          />
        ))}
        <div
          className={cx("ui-timeline__thumb", value === "live" && "ui-timeline__thumb--live")}
          style={{ top: `${pos(at) * 100}%` }}
          role="slider"
          tabIndex={0}
          aria-label={label}
          aria-orientation="vertical"
          aria-valuemin={from}
          aria-valuemax={to}
          aria-valuenow={at}
          aria-valuetext={value === "live" ? "Live" : formatTime(at)}
          onKeyDown={onKey}
        >
          {value === "live" ? "Live" : formatTime(at)}
        </div>
      </div>
      {value !== "live" && (
        <button
          type="button"
          className="ui-link ui-timeline__live"
          onClick={() => onChange("live")}
        >
          <Icon name="play" size={12} /> Back to live
        </button>
      )}
    </div>
  );
}
