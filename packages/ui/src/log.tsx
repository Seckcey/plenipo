/**
 * A read-only log: lines of program output as they arrive, in the monospace font, following the
 * newest line unless the reader scrolled up. Used by the terminal's watch tabs (Phase 12).
 */

import { useEffect, useRef, type ReactNode } from "react";

import { cx } from "./util";

export type LogTone = "normal" | "command" | "error" | "muted" | "ok" | "warn";

export interface LogLine {
  /** Stable key (the line's place in the log if omitted). */
  id?: string | undefined;
  text: ReactNode;
  tone?: LogTone | undefined;
}

export function LogView({
  label,
  lines,
  empty,
  className,
}: {
  label: string;
  lines: readonly LogLine[];
  /** Shown while there are no lines. */
  empty?: ReactNode;
  className?: string | undefined;
}) {
  const box = useRef<HTMLDivElement>(null);
  const following = useRef(true);
  useEffect(() => {
    const el = box.current;
    if (el && following.current) el.scrollTop = el.scrollHeight;
  }, [lines]);
  return (
    <div
      ref={box}
      className={cx("ui-log", className)}
      role="log"
      aria-label={label}
      tabIndex={0}
      onScroll={(e) => {
        const el = e.currentTarget;
        following.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
      }}
    >
      {lines.length === 0 ? (
        <div className="ui-log__empty">{empty}</div>
      ) : (
        lines.map((l, i) => (
          <div
            key={l.id ?? i}
            className={cx("ui-log__line", `ui-log__line--${l.tone ?? "normal"}`)}
          >
            {l.text}
          </div>
        ))
      )}
    </div>
  );
}
