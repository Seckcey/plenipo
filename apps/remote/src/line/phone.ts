import type {
  MeetingWelcome,
  NewPasskey,
  PairStep,
  PcSays,
  PhoneAsk,
  PhoneEvent,
  PhoneReply,
  PhoneSays,
} from "@plenipo/types";

import { decode, encode, fromUtf8, newId, utf8 } from "../lock/bytes";
import { mailboxOf, pskOf } from "../lock/code";
import { Assembler, Handshake, everydayPrologue, type KeyPair, type Line } from "../lock/noise";
import { LineEnded, Relay, type SocketMaker } from "./relay";

/**
 * The phone's side of the sealed line (ADR-141 to ADR-145): pairing with a code, then everyday
 * meetings, sign-in, and requests. Everything here is sealed for the PC alone.
 */

/** What the phone keeps after pairing (with its own key, in `keep.ts`). */
export interface Paired {
  /** The phone's ID on the PC. */
  device: string;
  /** The phone's ID at the relay, and its pass. */
  phone: string;
  pass: string;
  /** The PC: its relay key's fingerprint, its Noise key (base64url), and its name. */
  pc: string;
  pcKey: string;
  pcName: string;
  /** The passkey's ID. */
  credential: string;
}

/** The PC did not answer in time. */
export class NoAnswer extends Error {
  constructor(readonly request: string) {
    super("no answer from your PC");
  }
}

/** A request was sent, then the connection was lost: whether the PC got it is unknown. */
export class Lost extends Error {
  constructor(readonly request: string) {
    super("the connection was lost");
  }
}

async function sendSealed(relay: Relay, line: Line, message: PhoneSays): Promise<void> {
  for (const piece of await line.seal(utf8(JSON.stringify(message)))) relay.send(piece);
}

async function hearOne(
  relay: Relay,
  line: Line,
  assembler: Assembler,
  timeoutMs: number,
): Promise<PcSays> {
  for (;;) {
    const sealed = await relay.next(timeoutMs);
    const { more, bytes } = await line.open(sealed);
    const whole = assembler.add(more, bytes);
    if (whole) return JSON.parse(fromUtf8(whole)) as PcSays;
  }
}

/** Pairing with a code: the first meeting, the PC's "Is this your phone?", and the passkey. */
export class Pairing {
  private assembler = new Assembler();
  private pcKey: Uint8Array | null = null;

  private constructor(
    private readonly relay: Relay,
    private readonly line: Line,
  ) {}

  /**
   * Meet the PC waiting at the code's mailbox, and say what this phone is. Resolves when the PC
   * is asking its owner "Is this your phone?".
   */
  static async start(opts: {
    code: string;
    name: string;
    browser: string;
    keys: KeyPair;
    url?: string;
    make?: SocketMaker;
  }): Promise<Pairing> {
    const relay = await Relay.connect(
      { t: "mailbox", mailbox: await mailboxOf(opts.code) },
      opts.url,
      opts.make,
    );
    try {
      const hs = await Handshake.pairing({
        initiator: true,
        s: opts.keys,
        psk: await pskOf(opts.code),
      });
      relay.send(await hs.writeMessage(new Uint8Array(0)));
      await hs.readMessage(await relay.next());
      relay.send(
        await hs.writeMessage(utf8(JSON.stringify({ name: opts.name, browser: opts.browser }))),
      );
      const pairing = new Pairing(relay, await hs.open());
      pairing.pcKey = hs.rs;
      const first = await hearOne(relay, pairing.line, pairing.assembler, 30_000);
      if (first.t !== "pair" || first.pair.step !== "waiting") {
        throw new Error("Your PC did not answer as expected.");
      }
      return pairing;
    } catch (e) {
      relay.close();
      throw e;
    }
  }

  /** Wait for the owner's answer on the PC (up to 5 minutes). */
  async ownersAnswer(): Promise<PairStep> {
    const said = await hearOne(this.relay, this.line, this.assembler, 5 * 60_000);
    if (said.t !== "pair") throw new Error("Your PC did not answer as expected.");
    if (said.pair.step !== "accepted") this.relay.close();
    return said.pair;
  }

  /** Send the passkey just made; what the phone keeps when the PC says it is done. */
  async finish(
    accepted: Extract<PairStep, { step: "accepted" }>,
    passkey: NewPasskey,
  ): Promise<Paired> {
    try {
      await sendSealed(this.relay, this.line, { t: "passkey", passkey });
      const said = await hearOne(this.relay, this.line, this.assembler, 60_000);
      if (said.t !== "pair") throw new Error("Your PC did not answer as expected.");
      if (said.pair.step !== "done") {
        throw new Error(
          said.pair.step === "failed" || said.pair.step === "refused"
            ? said.pair.message
            : "Your PC did not finish adding this phone.",
        );
      }
      return {
        device: accepted.device,
        phone: accepted.phone,
        pass: accepted.pass,
        pc: accepted.pc,
        pcKey: encode(this.pcKey!),
        pcName: accepted.pcName,
        credential: passkey.id,
      };
    } finally {
      this.relay.close();
    }
  }

  cancel(): void {
    this.relay.close();
  }
}

type Pending = {
  resolve: (r: PhoneReply) => void;
  reject: (e: Error) => void;
  timer: ReturnType<typeof setTimeout>;
};

/** An everyday meeting with the PC: sign-in, requests, and what the PC tells the phone. */
export class Meeting {
  private assembler = new Assembler();
  private pending = new Map<string, Pending>();
  private eventListeners = new Set<(e: PhoneEvent) => void>();
  private endListeners = new Set<(e: LineEnded) => void>();
  private sending: Promise<void> = Promise.resolve();
  ended: LineEnded | null = null;

  private constructor(
    private readonly relay: Relay,
    private readonly line: Line,
    readonly welcome: MeetingWelcome,
  ) {}

  /** Meet the PC this phone was paired with. `notice`: sent by a notice's button. */
  static async open(opts: {
    paired: Paired;
    keys: KeyPair;
    notice?: boolean;
    url?: string;
    make?: SocketMaker;
  }): Promise<Meeting> {
    const relay = await Relay.connect({ t: "pass", pass: opts.paired.pass }, opts.url, opts.make);
    try {
      const pcKey = decode(opts.paired.pcKey);
      if (!pcKey || pcKey.length !== 32)
        throw new Error("This phone's record of your PC is damaged.");
      const hs = await Handshake.everyday({
        initiator: true,
        s: opts.keys,
        rs: pcKey,
        prologue: everydayPrologue(opts.paired.pc, opts.paired.phone),
      });
      relay.send(
        await hs.writeMessage(
          utf8(JSON.stringify({ notice: opts.notice ?? false, page: __PLENIPO_VERSION__ })),
        ),
      );
      const welcome = JSON.parse(
        fromUtf8(await hs.readMessage(await relay.next())),
      ) as MeetingWelcome;
      const meeting = new Meeting(relay, await hs.open(), welcome);
      // The meeting ends when `listen` reaches the end of the line: after everything the PC said
      // before it (a "removed" or "signed out" just before the line closes counts first).
      void meeting.listen();
      return meeting;
    } catch (e) {
      relay.close();
      throw e;
    }
  }

  private async listen(): Promise<void> {
    for (;;) {
      let said: PcSays;
      try {
        said = await hearOne(this.relay, this.line, this.assembler, 24 * 60 * 60_000);
      } catch (e) {
        this.endWith(e instanceof LineEnded ? e : new LineEnded("closed"));
        this.relay.close();
        return;
      }
      if (said.t === "reply") {
        const p = this.pending.get(said.reply.re);
        if (p) {
          clearTimeout(p.timer);
          this.pending.delete(said.reply.re);
          p.resolve(said.reply);
        }
      } else if (said.t === "event") {
        for (const l of this.eventListeners) l(said.event);
      }
    }
  }

  private endWith(e: LineEnded) {
    if (this.ended) return;
    this.ended = e;
    for (const [id, p] of this.pending) {
      clearTimeout(p.timer);
      p.reject(new Lost(id));
    }
    this.pending.clear();
    for (const l of this.endListeners) l(e);
  }

  /** Ask the PC. `again`: the same page read again after a change (not recorded again). */
  ask(
    ask: PhoneAsk,
    opts: { again?: boolean; id?: string; timeoutMs?: number } = {},
  ): Promise<PhoneReply> {
    const id = opts.id ?? newId();
    if (this.ended) return Promise.reject(new Lost(id));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new NoAnswer(id));
      }, opts.timeoutMs ?? 30_000);
      this.pending.set(id, { resolve, reject, timer });
      // Sealed one after another: each sealed message's counter goes out in order.
      this.sending = this.sending
        .then(() =>
          sendSealed(this.relay, this.line, { t: "ask", id, again: opts.again ?? false, ask }),
        )
        .catch(() => {
          clearTimeout(timer);
          this.pending.delete(id);
          reject(new Lost(id));
        });
    });
  }

  onEvent(listener: (e: PhoneEvent) => void): () => void {
    this.eventListeners.add(listener);
    return () => this.eventListeners.delete(listener);
  }

  onEnd(listener: (e: LineEnded) => void): () => void {
    if (this.ended) listener(this.ended);
    else this.endListeners.add(listener);
    return () => this.endListeners.delete(listener);
  }

  close(): void {
    this.relay.close();
  }
}
