/**
 * The frame around every page (ADR-030 §4): the left strip of sections (icons with names under
 * them, the owner's choice), the top bar, and the notice area.
 */

import type { ReactNode } from "react";

import { Select } from "./controls";
import { Icon, type IconName } from "./icons";
import { CountBadge, type Status } from "./status";
import type { ThemeName } from "./tokens";
import { cx } from "./util";

export function AppShell({
  rail,
  topBar,
  footer,
  children,
  className,
}: {
  rail: ReactNode;
  topBar: ReactNode;
  footer?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={cx("ui-shell", className)}>
      {rail}
      {topBar}
      {children}
      {footer}
    </div>
  );
}

export interface RailItem<Id extends string = string> {
  id: Id;
  label: string;
  icon: IconName;
  /** Longer description, shown as the tooltip. */
  tooltip?: string;
  badge?: { count: number; label: string; tone?: "accent" | Status };
  /** Pinned to the bottom of the strip (Settings, Diagnostics). */
  bottom?: boolean;
}

/** The left strip: one button per section, the active one marked. */
export function IconRail<Id extends string>({
  items,
  current,
  onSelect,
  brand,
  label = "Main",
}: {
  items: readonly RailItem<Id>[];
  current: Id;
  onSelect: (id: Id) => void;
  /** The mark at the top of the strip. */
  brand?: ReactNode;
  label?: string;
}) {
  const button = (item: RailItem<Id>) => (
    <li key={item.id}>
      <button
        type="button"
        className="ui-rail__item"
        data-tour={`nav-${item.id}`}
        aria-current={current === item.id ? "page" : undefined}
        title={item.tooltip ?? item.label}
        onClick={() => onSelect(item.id)}
      >
        <Icon name={item.icon} size={20} />
        <span className="ui-rail__label">{item.label}</span>
        {item.badge && (
          <CountBadge
            className="ui-rail__badge"
            count={item.badge.count}
            label={item.badge.label}
            tone={item.badge.tone ?? "accent"}
          />
        )}
      </button>
    </li>
  );
  return (
    <nav className="ui-rail" aria-label={label}>
      {brand && <div className="ui-rail__brand">{brand}</div>}
      <ul className="ui-rail__list">{items.filter((i) => !i.bottom).map(button)}</ul>
      <ul className="ui-rail__list ui-rail__list--bottom">
        {items.filter((i) => i.bottom).map(button)}
      </ul>
    </nav>
  );
}

export interface ScopeOption {
  id: string;
  label: string;
  kind: "organization" | "department" | "project";
}

/** Which part of the organization the pages show: all of it, a department, or a project. */
export function ScopeSelector({
  options,
  value,
  onChange,
}: {
  options: readonly ScopeOption[];
  value: string;
  onChange: (id: string) => void;
}) {
  const group = {
    organization: undefined,
    department: "Departments",
    project: "Projects",
  } as const;
  return (
    <div className="ui-scope">
      <Icon name="organization" size={14} />
      <Select
        label="Showing"
        hideLabel
        value={value}
        onChange={onChange}
        options={options.map((o) => ({
          value: o.id,
          label: o.label,
          ...(group[o.kind] ? { group: group[o.kind] } : {}),
        }))}
      />
    </div>
  );
}

export function TopBar({
  start,
  title,
  end,
}: {
  /** Left: the wordmark and the scope selector. */
  start?: ReactNode;
  title: ReactNode;
  /** Right: the theme switch, the bell, the version. */
  end?: ReactNode;
}) {
  return (
    <header className="ui-topbar">
      <div className="ui-topbar__start">{start}</div>
      {/* A label, not a heading: each page has its own heading. */}
      <div className="ui-topbar__title">{title}</div>
      <div className="ui-topbar__end">{end}</div>
    </header>
  );
}

/** Light/dark: the button names what it switches to. */
export function ThemeToggle({
  theme,
  onChange,
}: {
  theme: ThemeName;
  onChange: (next: ThemeName) => void;
}) {
  const next: ThemeName = theme === "dark" ? "light" : "dark";
  const label = next === "light" ? "Switch to the light theme" : "Switch to the dark theme";
  return (
    <button
      type="button"
      className="ui-icon-button ui-theme-toggle"
      aria-label={label}
      title={label}
      onClick={() => onChange(next)}
    >
      <Icon name={theme === "dark" ? "sun" : "moon"} size={16} />
    </button>
  );
}

/** The bell: how many things wait for you, and where to see them. */
export function NotificationBell({
  count,
  label,
  onOpen,
}: {
  count: number;
  /** Spoken with the count, e.g. "waiting for your approval". */
  label: string;
  onOpen: () => void;
}) {
  const spoken = count === 0 ? `Nothing ${label}` : `${count} ${label}`;
  return (
    <button
      type="button"
      className={cx("ui-icon-button", "ui-bell", count > 0 && "ui-bell--active")}
      aria-label={`Notifications: ${spoken}`}
      title={spoken}
      onClick={onOpen}
    >
      <Icon name="bell" size={16} />
      {count > 0 && (
        <span className="ui-bell__count ui-num" aria-hidden="true">
          {count > 99 ? "99+" : count}
        </span>
      )}
    </button>
  );
}

export type BannerTone = "info" | "ok" | "warn" | "error" | "pending";

/** One notice: an advisory or something you must do, with its button and Dismiss. */
export function Banner({
  tone = "info",
  title,
  children,
  action,
  onDismiss,
  role,
  className,
  label,
}: {
  tone?: BannerTone;
  title?: ReactNode;
  children?: ReactNode;
  /** The inline call to action, e.g. a Review button. */
  action?: ReactNode;
  onDismiss?: () => void;
  /** "alert" for things that must be seen now; "status" otherwise. */
  role?: "alert" | "status";
  className?: string;
  label?: string;
}) {
  const icon: IconName =
    tone === "error" || tone === "warn"
      ? "alert"
      : tone === "ok"
        ? "check"
        : tone === "pending"
          ? "clock"
          : "info";
  return (
    <div
      className={cx("ui-banner", `ui-banner--${tone}`, className)}
      role={role ?? (tone === "error" ? "alert" : "status")}
      aria-label={label}
    >
      <Icon name={icon} size={16} className="ui-banner__icon" />
      <div className="ui-banner__body">
        {title && <strong className="ui-banner__title">{title}</strong>}
        {children}
      </div>
      {(action || onDismiss) && (
        <div className="ui-banner__actions">
          {action}
          {onDismiss && (
            <button type="button" className="ui-link" onClick={onDismiss}>
              Dismiss
            </button>
          )}
        </div>
      )}
    </div>
  );
}

/** Where notices go, above the page. Empty, it takes no space. */
export function BannerSlot({ children }: { children: ReactNode }) {
  return (
    <div className="ui-banners" aria-label="Notices" role="region">
      {children}
    </div>
  );
}
