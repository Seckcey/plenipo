/**
 * A button that opens a short menu (the terminal's "New terminal": This PC, or a server), and a
 * handle to drag the edge of a panel (the terminal panel's size).
 */

import {
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from "react";

import type { ButtonVariant } from "./controls";
import { Icon, type IconName } from "./icons";
import { cx } from "./util";

export interface MenuItem {
  id: string;
  label: ReactNode;
  icon?: IconName | undefined;
  /** A second line or a mark after the label ("PRODUCTION"). */
  hint?: ReactNode;
  disabled?: boolean | undefined;
}

/** A menu behind a button: arrow keys move, Enter picks, Escape or a click outside closes. */
export function MenuButton({
  label,
  icon,
  items,
  onSelect,
  variant = "secondary",
  size = "sm",
  align = "start",
  empty = "Nothing to choose yet",
  title,
  placement = "below",
  defaultOpen = false,
}: {
  label: string;
  icon?: IconName | undefined;
  items: readonly MenuItem[];
  onSelect: (id: string) => void;
  variant?: ButtonVariant;
  size?: "sm" | "md";
  /** Which edge of the button the menu lines up with. */
  align?: "start" | "end";
  /** Said when there is nothing in the menu. */
  empty?: string;
  title?: string | undefined;
  /** Where the menu opens: below the button, or above it (near the bottom of the window). */
  placement?: "below" | "above";
  /** Open from the start (the Gallery shows it open). */
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const entries = useRef<(HTMLButtonElement | null)[]>([]);

  useEffect(() => {
    if (!open) return;
    const outside = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    // The document the menu is in: a popped-out panel's window has its own.
    const doc = root.current?.ownerDocument ?? document;
    doc.addEventListener("mousedown", outside);
    // The first item that can be picked takes the focus.
    entries.current.find((b) => b && !b.disabled)?.focus();
    return () => doc.removeEventListener("mousedown", outside);
  }, [open]);

  const close = () => {
    setOpen(false);
    button.current?.focus();
  };

  const move = (e: KeyboardEvent, index: number) => {
    const usable = entries.current.map((b, i) => (b && !b.disabled ? i : -1)).filter((i) => i >= 0);
    const at = usable.indexOf(index);
    let next: number | undefined;
    if (e.key === "ArrowDown") next = usable[(at + 1) % usable.length];
    else if (e.key === "ArrowUp") next = usable[(at - 1 + usable.length) % usable.length];
    else if (e.key === "Home") next = usable[0];
    else if (e.key === "End") next = usable[usable.length - 1];
    else if (e.key === "Escape" || e.key === "Tab") {
      if (e.key === "Escape") e.preventDefault();
      close();
      return;
    }
    if (next === undefined) return;
    e.preventDefault();
    entries.current[next]?.focus();
  };

  return (
    <div
      className={cx(
        "ui-menu",
        align === "end" && "ui-menu--end",
        placement === "above" && "ui-menu--above",
      )}
      ref={root}
      // Escape closes the menu even when nothing in it can take the focus, and so does the
      // focus leaving it (Tab away).
      onKeyDown={(e) => {
        if (open && e.key === "Escape") {
          e.preventDefault();
          close();
        }
      }}
      onBlur={(e) => {
        if (open && !root.current?.contains(e.relatedTarget)) setOpen(false);
      }}
    >
      <button
        ref={button}
        type="button"
        className={cx("ui-button", `ui-button--${variant}`, size === "sm" && "ui-button--sm")}
        aria-haspopup="menu"
        aria-expanded={open}
        title={title}
        onClick={() => setOpen((o) => !o)}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" && !open) {
            e.preventDefault();
            setOpen(true);
          }
        }}
      >
        {icon && <Icon name={icon} size={size === "sm" ? 14 : 16} />}
        {label}
        <Icon name="chevronDown" size={12} />
      </button>
      {open && (
        <div className="ui-menu__list" role="menu" aria-label={label}>
          {items.length === 0 ? (
            <div className="ui-menu__empty">{empty}</div>
          ) : (
            items.map((item, i) => (
              <button
                key={item.id}
                ref={(el) => {
                  entries.current[i] = el;
                }}
                type="button"
                role="menuitem"
                className="ui-menu__item"
                disabled={item.disabled}
                tabIndex={-1}
                onClick={() => {
                  setOpen(false);
                  button.current?.focus();
                  onSelect(item.id);
                }}
                onKeyDown={(e) => move(e, i)}
              >
                {item.icon && <Icon name={item.icon} size={14} />}
                <span className="ui-menu__label">{item.label}</span>
                {item.hint && <span className="ui-menu__hint">{item.hint}</span>}
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
}

/**
 * The edge of a panel, to drag (or move with the arrow keys) to resize it. `value` is the
 * panel's size in pixels; `edge` is where the handle is on the panel.
 */
export function ResizeHandle({
  label,
  value,
  min,
  max,
  edge,
  onChange,
  step = 16,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  /**
   * Where the handle is on the panel. "top": the panel is below (drag up to grow); "left": the
   * panel is to the right (drag left to grow); "right": the panel is to the left (drag right to
   * grow).
   */
  edge: "top" | "left" | "right";
  onChange: (next: number) => void;
  step?: number;
}) {
  const drag = useRef<{ start: number; from: number } | null>(null);
  const clamp = (n: number) => Math.round(Math.min(max, Math.max(min, n)));
  const at = (e: ReactPointerEvent) => (edge === "top" ? e.clientY : e.clientX);
  // Moving toward the panel's far side shrinks it: up and left grow a panel below or to the
  // right, and right grows a panel to the left.
  const sign = edge === "right" ? -1 : 1;
  return (
    <div
      role="separator"
      aria-label={label}
      aria-orientation={edge === "top" ? "horizontal" : "vertical"}
      aria-valuenow={Math.round(value)}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      className={cx("ui-resize", `ui-resize--${edge}`)}
      title={label}
      onPointerDown={(e) => {
        e.preventDefault();
        e.currentTarget.setPointerCapture(e.pointerId);
        drag.current = { start: at(e), from: value };
      }}
      onPointerMove={(e) => {
        if (!drag.current) return;
        onChange(clamp(drag.current.from + sign * (drag.current.start - at(e))));
      }}
      onPointerUp={(e) => {
        drag.current = null;
        e.currentTarget.releasePointerCapture(e.pointerId);
      }}
      onKeyDown={(e) => {
        const grow = edge === "top" ? "ArrowUp" : edge === "left" ? "ArrowLeft" : "ArrowRight";
        const shrink = edge === "top" ? "ArrowDown" : edge === "left" ? "ArrowRight" : "ArrowLeft";
        const next =
          e.key === grow
            ? value + step
            : e.key === shrink
              ? value - step
              : e.key === "Home"
                ? min
                : e.key === "End"
                  ? max
                  : null;
        if (next === null) return;
        e.preventDefault();
        onChange(clamp(next));
      }}
    />
  );
}
