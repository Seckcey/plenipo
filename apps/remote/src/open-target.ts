import { targetFromHash } from "./notice";

/**
 * The item a tapped notice opens (part 14C): from the page's address when the notice opened the
 * page (`#open=…`), or from the background part when the page was already open. One the page is
 * not ready for yet (it is still asking you to sign in) waits until it is.
 */

let initial: string | null =
  typeof window === "undefined" ? null : targetFromHash(window.location.hash);
if (initial && typeof history !== "undefined") {
  // Opened once: the address goes back to the page's own.
  history.replaceState(null, "", window.location.pathname);
}

const listeners = new Set<(target: string) => void>();

if (typeof navigator !== "undefined" && navigator.serviceWorker) {
  navigator.serviceWorker.addEventListener("message", (e: MessageEvent) => {
    const data = e.data as { type?: unknown; target?: unknown } | null;
    if (data?.type !== "plenipo-open" || typeof data.target !== "string") return;
    const target = targetFromHash(`#open=${encodeURIComponent(data.target)}`);
    if (!target) return;
    if (listeners.size === 0) initial = target;
    else for (const show of listeners) show(target);
  });
}

/** The item the page was opened on, once (then it is gone). */
export function takeInitialTarget(): string | null {
  const target = initial;
  initial = null;
  return target;
}

/** For the tests: as if the page had just opened on `target`. */
export function openedOn(target: string | null): void {
  initial = target;
}

/** Items the background part asks the open page to show (a notice tapped while it is open). */
export function subscribeOpen(show: (target: string) => void): () => void {
  listeners.add(show);
  return () => {
    listeners.delete(show);
  };
}
