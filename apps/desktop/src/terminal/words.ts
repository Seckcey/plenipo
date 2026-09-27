/** Small helpers for the terminal panel (Phase 12). */

/** The bytes in a base64 string (the shell's output, as the backend sent it). */
export function fromBase64(data: string): Uint8Array {
  const text = atob(data);
  const out = new Uint8Array(text.length);
  for (let i = 0; i < text.length; i++) out[i] = text.charCodeAt(i);
  return out;
}

/** How a terminal ended, for its last line. */
export function endedLine(why: string, code: number | null | undefined): string {
  const said = why.charAt(0).toUpperCase() + why.slice(1);
  return code !== undefined && code !== null && code !== 0
    ? `${said} (exit code ${code}).`
    : `${said}.`;
}

/**
 * The most characters sent to a terminal at once. Plenipo takes at most 64 KiB of typing in one
 * go, and a character here is at most 3 bytes (a pair, 4), so a large paste goes in pieces.
 */
export const TYPED_PIECE = 16_000;

/** `data` in pieces of at most `max` characters, never splitting a character in two. */
export function pieces(data: string, max = TYPED_PIECE): string[] {
  const out: string[] = [];
  let start = 0;
  while (start < data.length) {
    let end = Math.min(start + max, data.length);
    // Keep a pair (one character outside the basic plane, such as an emoji) together.
    const last = data.charCodeAt(end - 1);
    if (end < data.length && end - 1 > start && last >= 0xd800 && last <= 0xdbff) end -= 1;
    out.push(data.slice(start, end));
    start = end;
  }
  return out;
}

/** Where Settings → Terminal keeps "screen reader support" (this computer only). */
export const SCREEN_READER_KEY = "plenipo.terminal.screenReader";

/** Whether the owner asked for screen reader support in the terminal. */
export function screenReaderWanted(): boolean {
  try {
    return localStorage.getItem(SCREEN_READER_KEY) === "true";
  } catch {
    return false;
  }
}
