/** The entity card (Site Manager analogue) and the card grid with a card/list switch. */

import { useState, type ReactNode } from "react";

import type { ActivitySeries } from "./activity";
import { Segmented } from "./controls";
import { Icon, type IconName } from "./icons";
import { ErrorState, EmptyState, LoadingState, Skeleton } from "./states";
import { ActivityStrip, ActivityStripPlaceholder, StatusDot, type Status } from "./status";
import { cx, formatCount, useElementSize, useVirtualWindow } from "./util";

export interface Resource {
  icon: IconName;
  /** Spoken and shown as a tooltip, e.g. "Connect to servers". */
  label: string;
}

export interface EntityCardProps {
  title: string;
  status: Status;
  /** The status word, e.g. "Working". */
  statusLabel: string;
  /** Under the title, e.g. "Manager · Development". */
  subtype?: string;
  /** Activity: a series, still loading, failed to load, or none. */
  activity?: ActivitySeries | "loading" | "error" | null;
  now?: number;
  /** The provider/owner row, e.g. the AI tool that fills the position. */
  owner?: { icon: IconName; label: string };
  /** Small permission/resource icons along the bottom. */
  resources?: readonly Resource[];
  /** Said along the bottom when there are no icons (default: "No permissions"). */
  footerNote?: string;
  /** A small tag on the right of the title (e.g. "PRODUCTION"). */
  tag?: ReactNode;
  /** Makes the title a button that opens the entity. */
  onOpen?: () => void;
  selected?: boolean;
  className?: string;
}

export function EntityCard({
  title,
  status,
  statusLabel,
  subtype,
  activity,
  now,
  owner,
  resources,
  footerNote = "No permissions",
  tag,
  onOpen,
  selected = false,
  className,
}: EntityCardProps) {
  return (
    <article
      className={cx(
        "ui-card",
        selected && "ui-card--selected",
        onOpen && "ui-card--interactive",
        className,
      )}
      aria-label={`${title}, ${statusLabel}`}
    >
      <header className="ui-card__head">
        {onOpen ? (
          <button type="button" className="ui-card__title ui-card__open" onClick={onOpen}>
            {title}
          </button>
        ) : (
          <span className="ui-card__title">{title}</span>
        )}
        {tag}
      </header>
      <div className="ui-card__sub">
        <StatusDot status={status} label={statusLabel} />
        {subtype && <span className="ui-card__subtype">{subtype}</span>}
      </div>
      <div className="ui-card__activity">
        {activity === "loading" ? (
          <ActivityStripPlaceholder state="loading" />
        ) : activity === "error" ? (
          <ActivityStripPlaceholder state="error" />
        ) : activity ? (
          <ActivityStrip series={activity} now={now} label={`${title} activity`} />
        ) : (
          <ActivityStripPlaceholder state="empty" />
        )}
      </div>
      {owner && (
        <div className="ui-card__owner">
          <Icon name={owner.icon} size={14} />
          <span>{owner.label}</span>
        </div>
      )}
      <footer className="ui-card__foot">
        {resources && resources.length > 0 ? (
          <ul className="ui-card__resources" aria-label="Permissions">
            {resources.map((r) => (
              <li key={r.label} title={r.label}>
                <Icon name={r.icon} size={15} label={r.label} />
              </li>
            ))}
          </ul>
        ) : (
          <span className="ui-card__none">{footerNote}</span>
        )}
      </footer>
    </article>
  );
}

/** A card's shape while it loads. */
export function EntityCardSkeleton() {
  return (
    <div className="ui-card ui-card--skeleton" aria-hidden="true">
      <Skeleton width="60%" height={13} />
      <Skeleton width="40%" height={10} />
      <Skeleton height={8} />
      <Skeleton width="50%" height={10} />
    </div>
  );
}

export type CollectionView = "cards" | "list";

type CollectionState = "ready" | "loading" | "error";

const GAP = 8;
const LIST_ROW = 32;

/**
 * Cards in a responsive grid (as many columns as fit), or the same items as a list. Only the
 * rows on screen are drawn (ADR-030 §6), so hundreds of cards stay smooth.
 */
export function CardGrid<T>({
  label,
  items,
  getKey,
  renderCard,
  renderRow,
  view,
  onViewChange,
  state = "ready",
  error,
  onRetry,
  empty,
  height = 520,
  toolbar,
}: {
  label: string;
  items: readonly T[];
  getKey: (item: T) => string;
  renderCard: (item: T) => ReactNode;
  /** One line per item in list view. */
  renderRow: (item: T) => ReactNode;
  view: CollectionView;
  onViewChange: (view: CollectionView) => void;
  state?: CollectionState;
  error?: ReactNode;
  onRetry?: () => void;
  empty?: ReactNode;
  /** Height of the scrolling area (px). */
  height?: number;
  /** Extra controls on the right of the toolbar. */
  toolbar?: ReactNode;
}) {
  const [section, setSection] = useState<HTMLElement | null>(null);
  const [scroller, setScroller] = useState<HTMLDivElement | null>(null);
  const { width } = useElementSize(section);
  const cardWidth = 240;
  const cardHeight = 164;
  const columns =
    view === "cards"
      ? Math.max(1, Math.floor((Math.max(width, cardWidth) + GAP) / (cardWidth + GAP)))
      : 1;
  const rowHeight = view === "cards" ? cardHeight + GAP : LIST_ROW;
  const rows = Math.ceil(items.length / columns);
  const win = useVirtualWindow({ count: rows, itemHeight: rowHeight, scroller });
  const shown = items.slice(win.start * columns, win.end * columns);

  return (
    <section className="ui-collection" aria-label={label} ref={setSection}>
      <div className="ui-collection__toolbar">
        <span className="ui-collection__count ui-num">
          {state === "ready"
            ? `${formatCount(items.length)} ${items.length === 1 ? "item" : "items"}`
            : ""}
        </span>
        {toolbar}
        <Segmented
          label="View"
          value={view}
          onChange={onViewChange}
          options={[
            { value: "cards", label: "Cards", icon: "gallery", iconOnly: true },
            { value: "list", label: "List", icon: "list", iconOnly: true },
          ]}
        />
      </div>
      {state === "loading" ? (
        view === "cards" ? (
          <div
            className="ui-grid"
            style={{ gridTemplateColumns: `repeat(auto-fill, minmax(${cardWidth}px, 1fr))` }}
          >
            <span className="ui-visually-hidden" role="status">
              Loading…
            </span>
            {Array.from({ length: 6 }, (_, i) => (
              <EntityCardSkeleton key={i} />
            ))}
          </div>
        ) : (
          <LoadingState lines={5} />
        )
      ) : state === "error" ? (
        <ErrorState title="Couldn't load these" message={error} onRetry={onRetry} />
      ) : items.length === 0 ? (
        (empty ?? <EmptyState title="Nothing here yet" />)
      ) : (
        <div className="ui-collection__scroll" ref={setScroller} style={{ maxHeight: height }}>
          <div style={{ height: win.before }} aria-hidden="true" />
          {view === "cards" ? (
            <div
              className="ui-grid"
              style={{
                gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
                gridAutoRows: cardHeight,
              }}
            >
              {shown.map((item) => (
                <div key={getKey(item)} className="ui-grid__cell">
                  {renderCard(item)}
                </div>
              ))}
            </div>
          ) : (
            <ul className="ui-list">
              {shown.map((item) => (
                <li key={getKey(item)} className="ui-list__row" style={{ height: LIST_ROW }}>
                  {renderRow(item)}
                </li>
              ))}
            </ul>
          )}
          <div style={{ height: win.after }} aria-hidden="true" />
        </div>
      )}
    </section>
  );
}
