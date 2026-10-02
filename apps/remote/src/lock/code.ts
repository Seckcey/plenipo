import { buf, hex, utf8 } from "./bytes";

/**
 * Pairing codes (ADR-141 §2): 16 letters and digits, 80 bits, with no letters that look alike
 * (Crockford's base32: no I, L, O, or U). The phone reads one from the picture code (after `#`
 * in the pairing address, which a browser never sends to any server) or as the owner types it.
 *
 * The relay finds the waiting PC by a mailbox name made from the code with HKDF, never the code
 * itself, and the code becomes the first meeting's shared key (Noise's `psk`). The PC makes the
 * same two values (`crates/remote/src/code.rs`; both check `contracts/phone-relay/v1`).
 */

const ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
export const CODE_LENGTH = 16;
const SALT = "plenipo-remote-pairing.v1";

/** Read a code as typed: any case, with or without dashes and spaces; O is 0, I and L are 1. */
export function parseCode(typed: string): string | null {
  let out = "";
  for (const raw of typed) {
    if (raw === "-" || /\s/.test(raw)) continue;
    let c = raw.toUpperCase();
    if (c === "O") c = "0";
    if (c === "I" || c === "L") c = "1";
    if (!ALPHABET.includes(c) || c.length !== 1) return null;
    out += c;
    if (out.length > CODE_LENGTH) return null;
  }
  return out.length === CODE_LENGTH ? out : null;
}

/** As the PC shows it: four groups of four. */
export function shownCode(code: string): string {
  return code.match(/.{1,4}/g)?.join("-") ?? code;
}

/** The code in a pairing address's `#pair=…`, if any. */
export function codeFromHash(hash: string): string | null {
  const m = /^#pair=([0-9A-Za-z-]+)$/.exec(hash.trim());
  return m ? parseCode(m[1]!) : null;
}

async function derive(code: string, info: string, bytes: number): Promise<Uint8Array> {
  const key = await crypto.subtle.importKey("raw", buf(utf8(code)), "HKDF", false, ["deriveBits"]);
  const bits = await crypto.subtle.deriveBits(
    { name: "HKDF", hash: "SHA-256", salt: buf(utf8(SALT)), info: buf(utf8(info)) },
    key,
    bytes * 8,
  );
  return new Uint8Array(bits);
}

/** The relay's mailbox name for this code: 16 bytes from HKDF, in hex. */
export async function mailboxOf(code: string): Promise<string> {
  return hex(await derive(code, "mailbox", 16));
}

/** The first meeting's shared key. */
export async function pskOf(code: string): Promise<Uint8Array> {
  return derive(code, "psk", 32);
}
