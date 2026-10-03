import { useEffect, useId, useRef, type ReactNode } from "react";

import { StatusPill } from "./status";
import type { Status } from "./status-types";
import { cx, useStoredState } from "./util";

const isBoolean = (v: unknown): v is boolean => typeof v === "boolean";

/**
 * A card that opens and closes (Phase 25, item 2.2): its header always shows its name, a light,
 * one line of how it is, and its buttons; the rest shows only while it is open. It can remember
 * whether you left it open, and it opens by itself when it needs you.
 */
export function Disclosure({
  title,
  summary,
  status,
  actions,
  children,
  defaultOpen = false,
  openWhen = false,
  rememberAs,
  headingLevel = 3,
  headingId,
  label,
  className,
}: {
  title: ReactNode;
  /** One line under the name, shown open or closed: "Connected · 3 of 5 parts on". */
  summary?: ReactNode;
  /** The light beside the name. */
  status?: { status: Status; label: string } | undefined;
  /** Buttons in the header, usable while it is closed (they never open or close it). */
  actions?: ReactNode;
  children: ReactNode;
  /** Open to begin with, when nothing is remembered. */
  defaultOpen?: boolean;
  /** Opens it whenever this turns true: it needs you, or a link points at it. */
  openWhen?: boolean;
  /** Remembers whether it was left open, under this name (none: forgotten when it closes). */
  rememberAs?: string | undefined;
  headingLevel?: 2 | 3 | 4;
  /** The heading's ID, for a list item or link that names the card by it. */
  headingId?: string | undefined;
  /** Its name for screen readers, when the title is not plain words. */
  label?: string | undefined;
  className?: string | undefined;
}) {
  const [open, setOpen] = useStoredState<boolean>(
    rememberAs ? `disclosure:${rememberAs}` : undefined,
    defaultOpen,
    isBoolean,
  );
  const id = useId();
  const headId = headingId ?? `${id}-head`;
  const bodyId = `${id}-body`;
  // Opens when it starts needing you; closing it again is up to you.
  const needed = useRef(false);
  useEffect(() => {
    if (openWhen && !needed.current) setOpen(true);
    needed.current = openWhen;
  }, [openWhen, setOpen]);
  const Heading = `h${headingLevel}` as const;

  return (
    <section
      className={cx("ui-disclosure", open && "ui-disclosure--open", className)}
      aria-labelledby={label ? undefined : headId}
      aria-label={label}
    >
      <header className="ui-disclosure__head">
        <Heading className="ui-disclosure__title" id={headId}>
          <button
            type="button"
            className="ui-disclosure__toggle"
            aria-expanded={open}
            aria-controls={bodyId}
            onClick={() => setOpen(!open)}
          >
            <span className="ui-disclosure__chevron" aria-hidden="true" />
            <span className="ui-disclosure__name">{title}</span>
          </button>
        </Heading>
        {status && <StatusPill status={status.status} label={status.label} />}
        {actions && <div className="ui-disclosure__actions">{actions}</div>}
        {summary && <div className="ui-disclosure__summary">{summary}</div>}
      </header>
      <div className="ui-disclosure__body" id={bodyId} hidden={!open}>
        {open && children}
      </div>
    </section>
  );
}
