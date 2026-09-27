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
