/**
 * The detail split view (the UniFi port view): properties and switches on the left, a live
 * timeline in the middle, a map on the right, and a table below. Plus its parts: the property
 * list and the timeline scrubber.
 */

import { useRef, type KeyboardEvent, type ReactNode } from "react";

import { formatTime, HOUR_MS } from "./activity";
import { Icon } from "./icons";
import { EmptyState, ErrorState, LoadingState, Skeleton } from "./states";
import { cx } from "./util";
import type { Status } from "./status";

/** Whether a part has its data: loading, couldn't load, nothing to show, or ready. */
export type DetailState = "ready" | "loading" | "error" | "empty";

export function DetailSplitView({
  label,
  properties,
  timeline,
  map,
  table,
  state = "ready",
  error,
  onRetry,
  empty,
}: {
  label: string;
  properties: ReactNode;
  timeline?: ReactNode;
  map?: ReactNode;
  table?: ReactNode;
  state?: DetailState;
  error?: ReactNode;
  onRetry?: (() => void) | undefined;
  /** What to show when nothing is picked (default: "Nothing picked"). */
  empty?: ReactNode;
}) {
  if (state === "loading") {
    // One "Loading" for screen readers; the other panes are shapes only.
    return (
      <section className="ui-split" aria-label={label} aria-busy="true">
        <div className="ui-split__properties">
          <LoadingState label={`Loading ${label}`} lines={5} />
        </div>
        <div className="ui-split__timeline">
          <Skeleton height="100%" />
        </div>
        <div className="ui-split__map">
          <Skeleton height={160} />
        </div>
      </section>
    );
  }
  if (state === "error" || state === "empty") {
    return (
      <section className="ui-split ui-split--message" aria-label={label}>
        {state === "error" ? (
          <ErrorState title="Couldn't load the details" message={error} onRetry={onRetry} />
        ) : (
          (empty ?? (
            <EmptyState title="Nothing picked">
              Pick something from the list to see its details here.
            </EmptyState>
          ))
        )}
      </section>
    );
  }
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
  state = "ready",
  error,
  onRetry,
  empty = "Nothing to show yet",
}: {
  title?: ReactNode;
  items: readonly { label: string; value: ReactNode }[];
  state?: Exclude<DetailState, "empty">;
  error?: ReactNode;
  onRetry?: (() => void) | undefined;
  /** Shown when there are no rows. */
  empty?: string;
}) {
  if (state === "loading") {
    return (
      <div className="ui-props">
        {title && <div className="ui-props__title">{title}</div>}
        <LoadingState label="Loading details" lines={4} />
      </div>
    );
  }
  if (state === "error") {
    return (
      <div className="ui-props">
        {title && <div className="ui-props__title">{title}</div>}
        <ErrorState compact title="Couldn't load the details" message={error} onRetry={onRetry} />
      </div>
    );
  }
  if (items.length === 0) {
    return (
      <div className="ui-props">
        {title && <div className="ui-props__title">{title}</div>}
        <EmptyState compact title={empty} />
      </div>
    );
  }
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
  state = "ready",
  error,
  onRetry,
}: {
  from: number;
  to: number;
  events: readonly TimelineEvent[];
  value: TimelineValue;
  onChange: (next: TimelineValue) => void;
  label?: string;
  /** "empty" is not needed: with no events the head says so and the track still works. */
  state?: Exclude<DetailState, "empty">;
  error?: ReactNode;
  onRetry?: (() => void) | undefined;
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
  const head = `Last ${hours} ${hours === 1 ? "hour" : "hours"}`;

  if (state === "loading") {
    return (
      <div className="ui-timeline">
        <div className="ui-timeline__head">{head}</div>
        <LoadingState label="Loading the timeline" lines={6} />
      </div>
    );
  }
  if (state === "error") {
    return (
      <div className="ui-timeline">
        <div className="ui-timeline__head">{head}</div>
        <ErrorState compact title="Couldn't load the timeline" message={error} onRetry={onRetry} />
      </div>
    );
  }

  return (
    <div className="ui-timeline">
      <div className="ui-timeline__head">
        {head}{" "}
        <span className="ui-num">
          (
          {inRange.length === 0
            ? "no events yet"
            : `${inRange.length} ${inRange.length === 1 ? "event" : "events"}`}
          )
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
