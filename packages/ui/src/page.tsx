/**
 * The parts every page is made of (Phase 12): its header (with Back when another page opened
 * it), titled panels, short lists of things to open, number tiles, and Pip's greeting on Home.
 */

import type { ReactNode } from "react";

import type { PipPose } from "./brand-data";
import { Pip } from "./brand";
import { Button } from "./controls";
import { ErrorState, EmptyState, LoadingState } from "./states";
import { CountBadge, StatusDot, type Status } from "./status";
import { cx } from "./util";

/** A page's heading: what it is, its name, a line about it, its buttons, and Back. */
export function PageHeader({
  title,
  kicker,
  lead,
  status,
  actions,
  onBack,
  backLabel = "Back",
  id,
}: {
  title: ReactNode;
  /** What kind of page this is, above the name ("Department"). */
  kicker?: ReactNode;
  lead?: ReactNode;
  /** A status mark and word beside the name. */
  status?: ReactNode;
  actions?: ReactNode;
  /** Back to the page that opened this one. */
  onBack?: (() => void) | undefined;
  backLabel?: string;
  /** The heading's ID (for `aria-labelledby`). */
  id?: string | undefined;
}) {
  return (
    <header className="ui-page-head">
      {onBack && (
        <Button size="sm" variant="quiet" icon="chevronLeft" onClick={onBack}>
          {backLabel}
        </Button>
      )}
      <div className="ui-page-head__text">
        {kicker && <div className="ui-page-head__kicker">{kicker}</div>}
        <div className="ui-page-head__title">
          <h1 id={id}>{title}</h1>
          {status}
        </div>
        {lead && <p className="ui-page-head__lead">{lead}</p>}
      </div>
      {actions && <div className="ui-page-head__actions">{actions}</div>}
    </header>
  );
}

/** A titled part of a page. */
export function Panel({
  title,
  count,
  countLabel = "items",
  actions,
  children,
  id,
  className,
  wide = false,
}: {
  title: ReactNode;
  /** A count beside the title (with its spoken words). */
  count?: number | undefined;
  countLabel?: string;
  actions?: ReactNode;
  children: ReactNode;
  /** The heading's ID. */
  id: string;
  className?: string | undefined;
  /** Spans the whole width of a page grid. */
  wide?: boolean;
}) {
  return (
    <section className={cx("ui-panel", wide && "ui-panel--wide", className)} aria-labelledby={id}>
      <header className="ui-panel__head">
        <h2 id={id}>{title}</h2>
        {count !== undefined && <CountBadge count={count} label={countLabel} tone="accent" />}
        {actions && <div className="ui-panel__actions">{actions}</div>}
      </header>
      <div className="ui-panel__body">{children}</div>
    </section>
  );
}

/** One row of a short list: its name, a line under it, a status, and when. */
export interface RowItem {
  id: string;
  title: ReactNode;
  detail?: ReactNode;
  status?: { status: Status; label: string } | undefined;
  /** On the right: a time, a count. */
  meta?: ReactNode;
  /** A small tag after the name ("PRODUCTION"). */
  tag?: ReactNode;
  /** Opens its page (the row becomes a button). */
  onOpen?: (() => void) | undefined;
  /** Said by screen readers for the button, when the title is not enough. */
  openLabel?: string | undefined;
  /** Buttons at the end of the row, beside (not inside) the part that opens it: Watch, Stop. */
  actions?: ReactNode;
}

/** A short list of things to open (under about 30; longer lists are tables). */
export function RowList({
  label,
  items,
  state = "ready",
  error,
  onRetry,
  empty,
}: {
  label: string;
  items: readonly RowItem[];
  state?: "loading" | "error" | "ready";
  error?: ReactNode;
  onRetry?: (() => void) | undefined;
  /** What to show when there are no rows (an `EmptyState`, often with Pip). */
  empty?: ReactNode;
}) {
  if (state === "loading") return <LoadingState label={`Loading ${label}`} lines={3} />;
  if (state === "error") {
    return (
      <ErrorState compact title={`Couldn't load ${label}`} message={error} onRetry={onRetry} />
    );
  }
  if (items.length === 0) return <>{empty ?? <EmptyState compact title="Nothing yet" />}</>;
  return (
    <ul className="ui-rows" aria-label={label}>
      {items.map((item) => {
        const body = (
          <>
            <span className="ui-rows__main">
              <span className="ui-rows__title">
                {item.title}
                {item.tag}
              </span>
              {item.detail && <span className="ui-rows__detail">{item.detail}</span>}
            </span>
            {item.status && <StatusDot status={item.status.status} label={item.status.label} />}
            {item.meta && <span className="ui-rows__meta ui-num">{item.meta}</span>}
          </>
        );
        return (
          <li key={item.id} className={item.actions ? "ui-rows__row" : undefined}>
            {item.onOpen ? (
              <button
                type="button"
                className="ui-rows__item ui-rows__item--open"
                onClick={item.onOpen}
                aria-label={item.openLabel}
              >
                {body}
              </button>
            ) : (
              <div className="ui-rows__item">{body}</div>
            )}
            {item.actions && <span className="ui-rows__actions">{item.actions}</span>}
          </li>
        );
      })}
    </ul>
  );
}

/** A number and what it counts, as a small tile; with `onOpen`, it opens what it counts. */
export interface StatItem {
  label: string;
  value: ReactNode;
  status?: Status | undefined;
  hint?: ReactNode;
  onOpen?: (() => void) | undefined;
}

export function StatGrid({ label, stats }: { label: string; stats: readonly StatItem[] }) {
  return (
    <dl className="ui-stats" aria-label={label}>
      {stats.map((s) => {
        const inner = (
          <>
            <dt className="ui-stats__label">{s.label}</dt>
            <dd className="ui-stats__value ui-num">{s.value}</dd>
            {s.hint && <dd className="ui-stats__hint">{s.hint}</dd>}
          </>
        );
        return (
          <div
            key={s.label}
            className={cx("ui-stats__item", s.status && `ui-stats__item--${s.status}`)}
          >
            {s.onOpen ? (
              <button type="button" className="ui-stats__open" onClick={s.onOpen}>
                {inner}
              </button>
            ) : (
              inner
            )}
          </div>
        );
      })}
    </dl>
  );
}

/** Pip and a greeting, at the top of Home. */
export function Hero({
  pip,
  title,
  children,
  actions,
  id,
}: {
  pip: PipPose;
  title: ReactNode;
  children?: ReactNode;
  actions?: ReactNode;
  id?: string | undefined;
}) {
  return (
    <section className="ui-hero" aria-labelledby={id}>
      <Pip pose={pip} size="lg" className="ui-hero__pip" />
      <div className="ui-hero__text">
        <h1 id={id}>{title}</h1>
        {children && <div className="ui-hero__body">{children}</div>}
        {actions && <div className="ui-hero__actions">{actions}</div>}
      </div>
    </section>
  );
}
