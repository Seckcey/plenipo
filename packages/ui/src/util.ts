import { useCallback, useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";

/** Join class names, skipping empty ones. */
export function cx(...names: (string | false | null | undefined)[]): string {
  return names.filter(Boolean).join(" ");
}

function readStored<T>(key: string, fallback: T, valid: (v: unknown) => v is T): T {
  try {
    const raw = localStorage.getItem(key);
    if (raw === null) return fallback;
    const value: unknown = JSON.parse(raw);
    return valid(value) ? value : fallback;
  } catch {
    return fallback;
  }
}

/**
 * State remembered on this computer (`localStorage`), e.g. a table's hidden columns. Storage
 * that is unavailable or holds something unexpected falls back to `initial`.
 */
export function useStoredState<T>(
  key: string | undefined,
  initial: T,
  valid: (v: unknown) => v is T,
): [T, (next: T) => void] {
  const [value, setValue] = useState<T>(() => (key ? readStored(key, initial, valid) : initial));
  const set = useCallback(
    (next: T) => {
      setValue(next);
      if (!key) return;
      try {
        localStorage.setItem(key, JSON.stringify(next));
      } catch {
        // Storage unavailable: the choice lasts until the window closes.
      }
    },
    [key],
  );
  return [value, set];
}

export interface Size {
  width: number;
  height: number;
}

/**
 * The element's size, kept current with a ResizeObserver (zero until measured). Pass the
 * element itself (from a callback ref), so an element that appears later is measured too.
 */
export function useElementSize(el: HTMLElement | null): Size {
  const [size, setSize] = useState<Size>({ width: 0, height: 0 });
  useLayoutEffect(() => {
    if (!el) return;
    const measure = () => {
      const next = { width: el.clientWidth, height: el.clientHeight };
      setSize((prev) => (prev.width === next.width && prev.height === next.height ? prev : next));
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [el]);
  return size;
}

/** Viewport height assumed before layout (tests, first paint): about a screenful of rows. */
const FALLBACK_VIEWPORT = 720;

export interface VirtualWindow {
  /** First item to draw. */
  start: number;
  /** One past the last item to draw. */
  end: number;
  /** Space above the first drawn item (px). */
  before: number;
  /** Space below the last drawn item (px). */
  after: number;
}

/**
 * Windowing for fixed-height rows: only the rows in view (plus `overscan` on each side) are
 * drawn, so thousands of rows scroll smoothly (ADR-030 §6). `scroller` is the element that
 * scrolls; `offset` is how far the first row sits below its top (a sticky header).
 */
export function useVirtualWindow({
  count,
  itemHeight,
  scroller,
  overscan = 8,
  offset = 0,
}: {
  count: number;
  itemHeight: number;
  scroller: HTMLElement | null;
  overscan?: number;
  offset?: number;
}): VirtualWindow {
  // The scroll position belongs to one element: a scroller that is replaced (after loading, or
  // an error) starts at the top, as the browser draws it.
  const [scroll, setScroll] = useState<{ el: HTMLElement | null; top: number }>({
    el: null,
    top: 0,
  });
  const size = useElementSize(scroller);
  const frame = useRef<number | null>(null);

  useEffect(() => {
    const el = scroller;
    if (!el) return;
    const onScroll = () => {
      if (frame.current !== null) return;
      const schedule =
        typeof requestAnimationFrame === "function"
          ? requestAnimationFrame
          : (fn: FrameRequestCallback) => setTimeout(() => fn(0), 0) as unknown as number;
      frame.current = schedule(() => {
        frame.current = null;
        setScroll({ el, top: el.scrollTop });
      });
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      el.removeEventListener("scroll", onScroll);
      if (frame.current !== null && typeof cancelAnimationFrame === "function") {
        cancelAnimationFrame(frame.current);
      }
      frame.current = null;
    };
  }, [scroller]);

  const viewport = size.height > 0 ? size.height : FALLBACK_VIEWPORT;
  // A list that got shorter cannot be scrolled past its end (the browser clamps it too).
  const maxTop = Math.max(0, count * itemHeight - viewport);
  const scrollTop = scroll.el === scroller ? scroll.top : 0;
  const top = Math.min(maxTop, Math.max(0, scrollTop - offset));
  const first = Math.floor(top / itemHeight);
  const visible = Math.ceil(viewport / itemHeight) + 1;
  const start = Math.max(0, Math.min(count, first - overscan));
  const end = Math.max(start, Math.min(count, first + visible + overscan));
  return {
    start,
    end,
    before: start * itemHeight,
    after: (count - end) * itemHeight,
  };
}

/** Close a popover on Escape or a click outside `ref`. */
export function useDismiss(
  open: boolean,
  ref: RefObject<HTMLElement | null>,
  close: () => void,
): void {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    const onPointer = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) close();
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("mousedown", onPointer);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("mousedown", onPointer);
    };
  }, [open, ref, close]);
}

const numberFormat = new Intl.NumberFormat("en-US");

/** 5000 → "5,000". */
export const formatCount = (n: number) => numberFormat.format(n);
