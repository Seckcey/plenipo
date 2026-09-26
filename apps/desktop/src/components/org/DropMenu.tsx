import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";

export interface DropChoice {
  id: string;
  label: string;
  detail?: string;
  run: () => void;
}

/**
 * The choices after dropping a position on another one, at the drop point: report there, or
 * oversee its team as a reviewer, QA evaluator, or security auditor.
 */
export function DropMenu({
  title,
  x,
  y,
  choices,
  onClose,
}: {
  title: string;
  x: number;
  y: number;
  choices: DropChoice[];
  onClose: () => void;
}) {
  const menu = useRef<HTMLDivElement>(null);
  useEffect(() => {
    menu.current?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
    const away = (e: PointerEvent) => {
      if (!menu.current?.contains(e.target as Node)) onClose();
    };
    window.addEventListener("pointerdown", away, true);
    return () => window.removeEventListener("pointerdown", away, true);
  }, [onClose]);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const items = [...(menu.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])];
    const at = items.indexOf(document.activeElement as HTMLElement);
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      items[(at + 1) % items.length]?.focus();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      items[(at - 1 + items.length) % items.length]?.focus();
    }
  };

  // Keep the menu on screen once its size is known.
  const [place, setPlace] = useState({ left: x, top: y });
  useLayoutEffect(() => {
    const r = menu.current?.getBoundingClientRect();
    if (!r) return;
    setPlace({
      left: Math.max(8, Math.min(x, window.innerWidth - r.width - 8)),
      top: Math.max(8, Math.min(y, window.innerHeight - r.height - 8)),
    });
  }, [x, y, choices.length]);
  const { left, top } = place;
  return (
    <div
      ref={menu}
      className="drop-menu"
      role="menu"
      aria-label={title}
      style={{ left, top }}
      onKeyDown={onKeyDown}
    >
      <div className="drop-menu__title" aria-hidden="true">
        {title}
      </div>
      {choices.map((c) => (
        <button
          key={c.id}
          type="button"
          role="menuitem"
          className="drop-menu__item"
          onClick={() => {
            onClose();
            c.run();
          }}
        >
          <span>{c.label}</span>
          {c.detail && <span className="drop-menu__detail">{c.detail}</span>}
        </button>
      ))}
      <button
        type="button"
        role="menuitem"
        className="drop-menu__item drop-menu__item--cancel"
        onClick={onClose}
      >
        Cancel
      </button>
    </div>
  );
}
