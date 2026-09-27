/** Empty, loading (skeleton), and error states, shared by every component. */

import type { ReactNode } from "react";

import { Icon, type IconName } from "./icons";
import { cx } from "./util";

/** Nothing to show yet, and what to do about it. */
export function EmptyState({
  title,
  children,
  action,
  icon = "info",
  compact = false,
}: {
  title: string;
  children?: ReactNode;
  action?: ReactNode;
  icon?: IconName;
  compact?: boolean;
}) {
  return (
    <div className={cx("ui-empty", compact && "ui-empty--compact")} role="status">
      <Icon name={icon} size={compact ? 16 : 22} className="ui-empty__icon" />
      <div>
        <div className="ui-empty__title">{title}</div>
        {children && <div className="ui-empty__body">{children}</div>}
        {action && <div className="ui-empty__action">{action}</div>}
      </div>
    </div>
  );
}

/** Something went wrong: what, and a way to try again. */
export function ErrorState({
  title = "Something went wrong",
  message,
  onRetry,
  compact = false,
}: {
  title?: string;
  message?: ReactNode;
  onRetry?: (() => void) | undefined;
  compact?: boolean;
}) {
  return (
    <div className={cx("ui-error", compact && "ui-error--compact")} role="alert">
      <Icon name="alert" size={compact ? 16 : 22} className="ui-error__icon" />
      <div>
        <div className="ui-error__title">{title}</div>
        {message && <div className="ui-error__body">{message}</div>}
        {onRetry && (
          <button
            type="button"
            className="ui-button ui-button--secondary ui-button--sm"
            onClick={onRetry}
          >
            Try again
          </button>
        )}
      </div>
    </div>
  );
}

/** A grey placeholder shaped like the content that is loading. */
export function Skeleton({
  width = "100%",
  height = 12,
  radius,
  className,
}: {
  width?: number | string;
  height?: number | string;
  radius?: number;
  className?: string;
}) {
  return (
    <span
      className={cx("ui-skeleton", className)}
      style={{ width, height, borderRadius: radius }}
      aria-hidden="true"
    />
  );
}

/** Several skeleton lines, announced once as "Loading …". */
export function LoadingState({ label = "Loading", lines = 3 }: { label?: string; lines?: number }) {
  return (
    <div className="ui-loading" role="status" aria-live="polite">
      <span className="ui-visually-hidden">{label}…</span>
      {Array.from({ length: lines }, (_, i) => (
        <Skeleton key={i} width={`${90 - i * 17}%`} />
      ))}
    </div>
  );
}
