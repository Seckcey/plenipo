import { buf, concat, decode, utf8 } from "./bytes";

/**
 * The lock between the phone and the PC (ADR-143 §5): the Noise protocol, written on the browser's
 * own Web Crypto (X25519, AES-256-GCM, SHA-256), following the Noise specification (revision 34).
 *
 * - Pairing: `Noise_XXpsk3_25519_AESGCM_SHA256`, the pairing code as the shared key.
 * - Every meeting after: `Noise_KK_25519_AESGCM_SHA256`; each side already knows the other's key.
 *
 * The phone starts (the initiator). The PC answers with `snow` (`crates/remote/src/noise.rs`). Both
 * sides are checked against the same fixed test answers (`contracts/phone-relay/v1/noise-vectors.json`).
 *
 * The phone's own long-term key is made so it can never be copied out of the browser
 * (non-extractable): only Web Crypto's `deriveBits` ever uses it.
 */

export const PAIRING = "Noise_XXpsk3_25519_AESGCM_SHA256";
export const EVERYDAY = "Noise_KK_25519_AESGCM_SHA256";
export const PAIRING_PROLOGUE = utf8("plenipo-remote.v1/pair");
export const TAG = 16;
export const MAX_MESSAGE = 65_535;
/** A piece that more follow, and the last piece. */
export const MORE = 1;
export const LAST = 0;
/** The most plain bytes in one piece, after its first byte. */
export const PIECE = MAX_MESSAGE - TAG - 1;
/** The most bytes put back together from pieces. */
export const MAX_ASSEMBLED = 4 * 1024 * 1024;

/** What both sides agree on before an everyday meeting: the PC and the phone it is for. */
export function everydayPrologue(pc: string, phone: string): Uint8Array {
  return utf8(`plenipo-remote.v1/kk/${pc}/${phone}`);
}

/** A key pair: the private half stays a Web Crypto key; the public half is its 32 bytes. */
export interface KeyPair {
  privateKey: CryptoKey;
  publicKey: Uint8Array;
}

/** A new X25519 key pair. `extractable: false` for the phone's long-term key. */
export async function newKeyPair(extractable = false): Promise<KeyPair> {
  const pair = await crypto.subtle.generateKey({ name: "X25519" }, extractable, ["deriveBits"]);
  const publicKey = new Uint8Array(await crypto.subtle.exportKey("raw", pair.publicKey));
  return { privateKey: pair.privateKey, publicKey };
}

/** The PKCS #8 start of an X25519 private key, before its 32 bytes. */
const X25519_PKCS8 = new Uint8Array([
  0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x04, 0x22, 0x04, 0x20,
]);

/** A key pair from fixed private bytes (the tests' fixed keys only). */
export async function fixedKeyPair(privateBytes: Uint8Array): Promise<KeyPair> {
  const privateKey = await crypto.subtle.importKey(
    "pkcs8",
    buf(concat(X25519_PKCS8, privateBytes)),
    { name: "X25519" },
    true,
    ["deriveBits"],
  );
  const jwk = await crypto.subtle.exportKey("jwk", privateKey);
  const publicKey = decode(jwk.x ?? "");
  if (!publicKey || publicKey.length !== 32) throw new Error("not an X25519 key");
  return { privateKey, publicKey };
}

async function dh(own: KeyPair, theirs: Uint8Array): Promise<Uint8Array> {
  const pub = await crypto.subtle.importKey("raw", buf(theirs), { name: "X25519" }, true, []);
  return new Uint8Array(
    await crypto.subtle.deriveBits({ name: "X25519", public: pub }, own.privateKey, 256),
  );
}

async function sha256(data: Uint8Array): Promise<Uint8Array> {
  return new Uint8Array(await crypto.subtle.digest("SHA-256", buf(data)));
}

async function hmac(key: Uint8Array, data: Uint8Array): Promise<Uint8Array> {
  const k = await crypto.subtle.importKey(
    "raw",
    buf(key),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  return new Uint8Array(await crypto.subtle.sign("HMAC", k, buf(data)));
}

/** Noise's HKDF: two or three 32-byte outputs. */
async function hkdf(ck: Uint8Array, ikm: Uint8Array, n: 2 | 3): Promise<Uint8Array[]> {
  const temp = await hmac(ck, ikm);
  const one = await hmac(temp, new Uint8Array([1]));
  const two = await hmac(temp, concat(one, new Uint8Array([2])));
  if (n === 2) return [one, two];
  const three = await hmac(temp, concat(two, new Uint8Array([3])));
  return [one, two, three];
}

/** One direction's key and counter. */
export class CipherState {
  private key: CryptoKey | null = null;
  private n = 0n;

  async initializeKey(raw: Uint8Array): Promise<void> {
    this.key = await crypto.subtle.importKey("raw", buf(raw), "AES-GCM", false, [
      "encrypt",
      "decrypt",
    ]);
    this.n = 0n;
  }

  hasKey(): boolean {
    return this.key !== null;
  }

  private nonce(): Uint8Array {
    const iv = new Uint8Array(12);
    new DataView(iv.buffer).setBigUint64(4, this.n, false);
    return iv;
  }

  async encrypt(ad: Uint8Array, plain: Uint8Array): Promise<Uint8Array> {
    if (!this.key) return plain;
    if (this.n >= 0xffffffffffffffffn) throw new Error("the line has run out of numbers");
    const sealed = await crypto.subtle.encrypt(
      { name: "AES-GCM", iv: buf(this.nonce()), additionalData: buf(ad), tagLength: 128 },
      this.key,
      buf(plain),
    );
    this.n += 1n;
    return new Uint8Array(sealed);
  }

  async decrypt(ad: Uint8Array, sealed: Uint8Array): Promise<Uint8Array> {
    if (!this.key) return sealed;
    const plain = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv: buf(this.nonce()), additionalData: buf(ad), tagLength: 128 },
      this.key,
      buf(sealed),
    );
    this.n += 1n;
    return new Uint8Array(plain);
  }
}

class SymmetricState {
  ck: Uint8Array = new Uint8Array(32);
  h: Uint8Array = new Uint8Array(32);
  cipher = new CipherState();

  async init(name: string): Promise<void> {
    const bytes = utf8(name);
    if (bytes.length <= 32) {
      this.h = new Uint8Array(32);
      this.h.set(bytes);
    } else {
      this.h = await sha256(bytes);
    }
    this.ck = this.h.slice();
  }

  async mixHash(data: Uint8Array): Promise<void> {
    this.h = await sha256(concat(this.h, data));
  }

  async mixKey(ikm: Uint8Array): Promise<void> {
    const [ck, k] = await hkdf(this.ck, ikm, 2);
    this.ck = ck!;
    await this.cipher.initializeKey(k!);
  }

  async mixKeyAndHash(ikm: Uint8Array): Promise<void> {
    const [ck, h, k] = await hkdf(this.ck, ikm, 3);
    this.ck = ck!;
    await this.mixHash(h!);
    await this.cipher.initializeKey(k!);
  }

  async encryptAndHash(plain: Uint8Array): Promise<Uint8Array> {
    const sealed = await this.cipher.encrypt(this.h, plain);
    await this.mixHash(sealed);
    return sealed;
  }

  async decryptAndHash(sealed: Uint8Array): Promise<Uint8Array> {
    const plain = await this.cipher.decrypt(this.h, sealed);
    await this.mixHash(sealed);
    return plain;
  }

  async split(): Promise<[CipherState, CipherState]> {
    const [one, two] = await hkdf(this.ck, new Uint8Array(0), 2);
    const a = new CipherState();
    const b = new CipherState();
    await a.initializeKey(one!);
    await b.initializeKey(two!);
    return [a, b];
  }
}

type Token = "e" | "s" | "ee" | "es" | "se" | "ss" | "psk";

const PATTERNS: Record<"XXpsk3" | "KK", Token[][]> = {
  XXpsk3: [["e"], ["e", "ee", "s", "es"], ["s", "se", "psk"]],
  KK: [
    ["e", "es", "ss"],
    ["e", "ee", "se"],
  ],
};

/** A meeting, from the phone's side (the initiator; the PC's side exists for the tests). */
export class Handshake {
  private ss = new SymmetricState();
  private messages: Token[][];
  private index = 0;
  private e: KeyPair | null = null;
  re: Uint8Array | null = null;
  rs: Uint8Array | null;

  private constructor(
    pattern: "XXpsk3" | "KK",
    private readonly initiator: boolean,
    private readonly s: KeyPair,
    rs: Uint8Array | null,
    private readonly psk: Uint8Array | null,
    private fixedE: KeyPair | null,
  ) {
    this.messages = PATTERNS[pattern];
    this.rs = rs;
  }

  /** A first meeting, with the code's shared key. */
  static async pairing(opts: {
    initiator: boolean;
    s: KeyPair;
    psk: Uint8Array;
    e?: KeyPair;
  }): Promise<Handshake> {
    const hs = new Handshake("XXpsk3", opts.initiator, opts.s, null, opts.psk, opts.e ?? null);
    await hs.ss.init(PAIRING);
    await hs.ss.mixHash(PAIRING_PROLOGUE);
    return hs;
  }

  /** An everyday meeting: each side knows the other's long-term key. */
  static async everyday(opts: {
    initiator: boolean;
    s: KeyPair;
    rs: Uint8Array;
    prologue: Uint8Array;
    e?: KeyPair;
  }): Promise<Handshake> {
    const hs = new Handshake("KK", opts.initiator, opts.s, opts.rs, null, opts.e ?? null);
    await hs.ss.init(EVERYDAY);
    await hs.ss.mixHash(opts.prologue);
    // Pre-messages: the initiator's key, then the responder's.
    const [first, second] = opts.initiator
      ? [opts.s.publicKey, opts.rs]
      : [opts.rs, opts.s.publicKey];
    await hs.ss.mixHash(first);
    await hs.ss.mixHash(second);
    return hs;
  }

  get finished(): boolean {
    return this.index >= this.messages.length;
  }

  get handshakeHash(): Uint8Array {
    return this.ss.h;
  }

  /** Is the next message this side's to write? */
  get myTurn(): boolean {
    return (this.index % 2 === 0) === this.initiator;
  }

  private async dhFor(token: "ee" | "es" | "se" | "ss"): Promise<Uint8Array> {
    const need = (k: Uint8Array | null | KeyPair, what: string) => {
      if (!k) throw new Error(`the meeting has no ${what}`);
      return k;
    };
    switch (token) {
      case "ee":
        return dh(
          need(this.e, "ephemeral") as KeyPair,
          need(this.re, "remote ephemeral") as Uint8Array,
        );
      case "ss":
        return dh(this.s, need(this.rs, "remote static") as Uint8Array);
      case "es":
        return this.initiator
          ? dh(need(this.e, "ephemeral") as KeyPair, need(this.rs, "remote static") as Uint8Array)
          : dh(this.s, need(this.re, "remote ephemeral") as Uint8Array);
      case "se":
        return this.initiator
          ? dh(this.s, need(this.re, "remote ephemeral") as Uint8Array)
          : dh(need(this.e, "ephemeral") as KeyPair, need(this.rs, "remote static") as Uint8Array);
    }
  }

  async writeMessage(payload: Uint8Array): Promise<Uint8Array> {
    if (this.finished || !this.myTurn) throw new Error("not this side's turn");
    const parts: Uint8Array[] = [];
    for (const token of this.messages[this.index]!) {
      switch (token) {
        case "e": {
          this.e = this.fixedE ?? (await newKeyPair(false));
          this.fixedE = null;
          parts.push(this.e.publicKey);
          await this.ss.mixHash(this.e.publicKey);
          if (this.psk) await this.ss.mixKey(this.e.publicKey);
          break;
        }
        case "s":
          parts.push(await this.ss.encryptAndHash(this.s.publicKey));
          break;
        case "psk":
          await this.ss.mixKeyAndHash(this.psk!);
          break;
        default:
          await this.ss.mixKey(await this.dhFor(token));
      }
    }
    parts.push(await this.ss.encryptAndHash(payload));
    this.index += 1;
    const message = concat(...parts);
    if (message.length > MAX_MESSAGE) throw new Error("a meeting message was too long");
    return message;
  }

  async readMessage(message: Uint8Array): Promise<Uint8Array> {
    if (this.finished || this.myTurn) throw new Error("not the other side's turn");
    if (message.length > MAX_MESSAGE) throw new Error("a meeting message was too long");
    let at = 0;
    const take = (n: number) => {
      if (at + n > message.length) throw new Error("a meeting message was too short");
      const part = message.slice(at, at + n);
      at += n;
      return part;
    };
    for (const token of this.messages[this.index]!) {
      switch (token) {
        case "e": {
          this.re = take(32);
          await this.ss.mixHash(this.re);
          if (this.psk) await this.ss.mixKey(this.re);
          break;
        }
        case "s": {
          const sealed = take(this.ss.cipher.hasKey() ? 32 + TAG : 32);
          this.rs = await this.ss.decryptAndHash(sealed);
          break;
        }
        case "psk":
          await this.ss.mixKeyAndHash(this.psk!);
          break;
        default:
          await this.ss.mixKey(await this.dhFor(token));
      }
    }
    const payload = await this.ss.decryptAndHash(message.slice(at));
    this.index += 1;
    return payload;
  }

  /** The open line, after the last message: this side's sending and receiving halves. */
  async open(): Promise<Line> {
    if (!this.finished) throw new Error("the meeting is not finished");
    const [first, second] = await this.ss.split();
    return this.initiator ? new Line(first, second) : new Line(second, first);
  }
}

/** The open line: every sealed message carries a counter, so a copy or a change fails. */
export class Line {
  constructor(
    private readonly sending: CipherState,
    private readonly receiving: CipherState,
  ) {}

  /** Seal a whole message, in as many pieces as it needs. */
  async seal(message: Uint8Array): Promise<Uint8Array[]> {
    if (message.length > MAX_ASSEMBLED) throw new Error("a message was too long to send");
    const pieces: Uint8Array[] = [];
    if (message.length === 0) {
      pieces.push(await this.sending.encrypt(new Uint8Array(0), new Uint8Array([LAST])));
      return pieces;
    }
    for (let at = 0; at < message.length; at += PIECE) {
      const chunk = message.slice(at, at + PIECE);
      const flag = at + PIECE < message.length ? MORE : LAST;
      pieces.push(
        await this.sending.encrypt(new Uint8Array(0), concat(new Uint8Array([flag]), chunk)),
      );
    }
    return pieces;
  }

  /** Open one sealed piece. Throws when it was copied, changed, out of order, or not ours. */
  async open(sealed: Uint8Array): Promise<{ more: boolean; bytes: Uint8Array }> {
    if (sealed.length > MAX_MESSAGE || sealed.length < TAG + 1) throw new Error("wrong size");
    const plain = await this.receiving.decrypt(new Uint8Array(0), sealed);
    const flag = plain[0];
    if (flag !== MORE && flag !== LAST) throw new Error("not one of ours");
    return { more: flag === MORE, bytes: plain.slice(1) };
  }
}

/** Puts pieces back together. */
export class Assembler {
  private parts: Uint8Array[] = [];
  private size = 0;

  add(more: boolean, bytes: Uint8Array): Uint8Array | null {
    this.size += bytes.length;
    if (this.size > MAX_ASSEMBLED) {
      this.parts = [];
      this.size = 0;
      throw new Error("a message was too long");
    }
    this.parts.push(bytes);
    if (more) return null;
    const whole = concat(...this.parts);
    this.parts = [];
    this.size = 0;
    return whole;
  }
}
