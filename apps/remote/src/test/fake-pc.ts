import type { PcSays, PhoneAsk, PhoneReply, PhoneSays } from "@plenipo/types";

import { decode, encode, fromUtf8, utf8 } from "../lock/bytes";
import { mailboxOf, pskOf } from "../lock/code";
import {
  Assembler,
  Handshake,
  everydayPrologue,
  newKeyPair,
  type KeyPair,
  type Line,
} from "../lock/noise";

/**
 * A stand-in for the PC and the relay, for the page's tests: it pairs a phone with a code, meets it
 * (Noise, the same lock as the real PC), signs it in, and answers its requests with `answer`.
 * The real PC's side is tested in Rust (`crates/remote`); this checks the page.
 */
export class FakePc {
  keys!: KeyPair;
  fingerprint = "pc-fingerprint";
  pcName = "Office PC";
  code = "7K3QM9TX2HFDR8WB";
  /** Paired phones: relay ID → their long-term key. */
  phones = new Map<string, Uint8Array>();
  signedIn = new Set<string>();
  online = true;
  removed = new Set<string>();
  /** Phones removed while they were away: the relay still takes their pass, once. */
  forgotten = new Set<string>();
  answer: (ask: PhoneAsk) => unknown = () => ({});
  asked: PhoneAsk[] = [];
  /** The pairing's next step: what the owner says to "Is this your phone?". */
  ownerSays: "add" | "no" = "add";
  sockets: FakeSocket[] = [];
  /** What the phone said it is, when pairing. */
  hello: { name: string; browser: string } | null = null;
  /** The PC's notice key (part 14C): 65 bytes, as a P-256 public key is sent. */
  noticeKey: string | null = encode(new Uint8Array(65).fill(4));

  static async start(): Promise<FakePc> {
    const pc = new FakePc();
    pc.keys = await newKeyPair(true);
    return pc;
  }

  make = (url: string): WebSocket => {
    const s = new FakeSocket(this, url);
    this.sockets.push(s);
    return s as unknown as WebSocket;
  };

  /** Tell every signed-in phone something changed. */
  async tell(what: string, org = "first"): Promise<void> {
    for (const s of this.sockets) await s.event({ kind: "changed", org, what });
  }

  /** The PC goes away (its phones are told, as the relay tells them). */
  goOffline(): void {
    this.online = false;
    for (const s of this.sockets) s.pcGone();
  }

  /** Phone access switched off on the PC: its phones are signed out, then the PC leaves. */
  async switchOff(): Promise<void> {
    for (const s of this.sockets) await s.event({ kind: "signedOut", why: "switchedOff" });
    this.goOffline();
  }

  /** The phone removed on the PC: told, and its connection closed right after. */
  async removeAndClose(phone = "cGhvbmUtMQ"): Promise<void> {
    for (const s of this.sockets) await s.event({ kind: "signedOut", why: "removed" });
    this.removed.add(phone);
    for (const s of this.sockets) s.close();
  }
}

export class FakeSocket {
  onopen: (() => void) | null = null;
  onmessage: ((e: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  private hs: Handshake | null = null;
  private line: Line | null = null;
  private assembler = new Assembler();
  private phone: string | null = null;
  private challenge: string | null = null;
  private pairingKey: Uint8Array | null = null;
  private closed = false;
  private dropOnData = false;
  /** Met from a notice: only a no is taken (ADR-142 §5). */
  private fromNotice = false;
  private queue: Promise<void> = Promise.resolve();

  constructor(
    private readonly pc: FakePc,
    readonly url: string,
  ) {
    setTimeout(() => this.onopen?.(), 0);
  }

  private say(message: object) {
    if (this.closed) return;
    const data = JSON.stringify(message);
    setTimeout(() => this.onmessage?.({ data }), 0);
  }

  close() {
    if (this.closed) return;
    this.closed = true;
    setTimeout(() => this.onclose?.(), 0);
  }

  pcGone() {
    this.say({ t: "pc_offline" });
    this.close();
  }

  private async sendSealed(message: PcSays) {
    for (const piece of await this.line!.seal(utf8(JSON.stringify(message)))) {
      this.say({ t: "data", data: encode(piece) });
    }
  }

  async event(event: object) {
    if (this.line && this.phone && this.pc.signedIn.has(this.phone)) {
      await this.sendSealed({ t: "event", event } as PcSays);
    }
  }

  send(text: string) {
    this.queue = this.queue.then(() => this.receive(text)).catch(() => this.close());
  }

  private async receive(text: string) {
    const m = JSON.parse(text) as { t: string; pass?: string; mailbox?: string; data?: string };
    if (m.t === "pass") {
      const [phone] = (m.pass ?? "").split(".");
      if (!this.pc.online) return this.refuse("pc_offline");
      if (phone && this.pc.forgotten.has(phone)) {
        // The PC does not know it: it tells the relay to refuse the pass, after the meeting began.
        this.say({ t: "ready" });
        this.dropOnData = true;
        return;
      }
      if (!phone || this.pc.removed.has(phone) || !this.pc.phones.has(phone)) {
        return this.refuse("bad_pass");
      }
      this.phone = phone;
      this.hs = await Handshake.everyday({
        initiator: false,
        s: this.pc.keys,
        rs: this.pc.phones.get(phone)!,
        prologue: everydayPrologue(this.pc.fingerprint, phone),
      });
      this.say({ t: "ready" });
      return;
    }
    if (m.t === "mailbox") {
      if (!this.pc.online || m.mailbox !== (await mailboxOf(this.pc.code))) {
        return this.refuse("mailbox_closed");
      }
      this.hs = await Handshake.pairing({
        initiator: false,
        s: this.pc.keys,
        psk: await pskOf(this.pc.code),
      });
      this.say({ t: "ready" });
      return;
    }
    if (m.t !== "data" || !m.data) return;
    if (this.dropOnData) return this.refuse("bad_pass");
    const bytes = decode(m.data)!;
    if (this.hs && !this.hs.finished) {
      const payload = await this.hs.readMessage(bytes);
      if (this.phone) {
        // An everyday meeting: answer with the welcome. One from a notice is never signed in,
        // and gets no challenge: it may only say no.
        const hello = payload.length ? (JSON.parse(fromUtf8(payload)) as { notice?: boolean }) : {};
        this.fromNotice = hello.notice === true;
        const signedIn = this.pc.signedIn.has(this.phone) && !this.fromNotice;
        this.challenge =
          signedIn || this.fromNotice ? null : encode(crypto.getRandomValues(new Uint8Array(32)));
        const welcome = {
          signedIn,
          ...(this.challenge ? { challenge: this.challenge } : {}),
          pcName: this.pc.pcName,
          version: "1.19.2",
          ...(this.pc.noticeKey ? { noticeKey: this.pc.noticeKey } : {}),
        };
        this.say({
          t: "data",
          data: encode(await this.hs.writeMessage(utf8(JSON.stringify(welcome)))),
        });
        this.line = await this.hs.open();
        return;
      }
      if (!this.hs.finished) {
        // The first meeting's second message.
        this.say({ t: "data", data: encode(await this.hs.writeMessage(new Uint8Array(0))) });
        return;
      }
      // The third: the code was right, and it says what the phone calls itself.
      this.pc.hello = JSON.parse(fromUtf8(payload)) as { name: string; browser: string };
      this.pairingKey = this.hs.rs;
      this.line = await this.hs.open();
      await this.sendSealed({ t: "pair", pair: { step: "waiting" } });
      setTimeout(() => void this.ownerAnswers(), 10);
      return;
    }
    const { more, bytes: plain } = await this.line!.open(bytes);
    const whole = this.assembler.add(more, plain);
    if (!whole) return;
    const said = JSON.parse(fromUtf8(whole)) as PhoneSays;
    if (said.t === "passkey") {
      const phone = "cGhvbmUtMQ";
      this.pc.phones.set(phone, this.pairingKey!);
      this.pc.signedIn.add(phone);
      await this.sendSealed({ t: "pair", pair: { step: "done" } });
      return;
    }
    const ask = said.ask;
    if (ask.kind === "signIn") {
      this.pc.signedIn.add(this.phone!);
      await this.reply(said.id, { ok: { pass: `${this.phone}.renewed`, endsAt: Date.now() + 1 } });
      return;
    }
    if (this.fromNotice) {
      if (ask.kind !== "refuse" && ask.kind !== "discardLesson") {
        await this.reply(said.id, {
          refused: { why: "notFromANotice", message: "Open Plenipo on your phone to do that." },
        });
        return;
      }
    } else if (!this.pc.signedIn.has(this.phone!)) {
      await this.reply(said.id, {
        refused: { why: "notSignedIn", message: "Sign in on this phone first." },
      });
      return;
    }
    this.pc.asked.push(ask);
    if (ask.kind === "signOut") this.pc.signedIn.delete(this.phone!);
    if (ask.kind === "removeThisPhone") this.pc.removed.add(this.phone!);
    const out = this.pc.answer(ask);
    if (out && typeof out === "object" && "refused" in out && typeof out.refused === "string") {
      await this.reply(said.id, { refused: { message: out.refused } });
    } else {
      await this.reply(said.id, { ok: out ?? {} });
    }
  }

  private async ownerAnswers() {
    if (this.pc.ownerSays === "no") {
      await this.sendSealed({
        t: "pair",
        pair: { step: "refused", message: "Your PC said this is not your phone." },
      });
      return;
    }
    await this.sendSealed({
      t: "pair",
      pair: {
        step: "accepted",
        device: "ZGV2aWNlLTE",
        phone: "cGhvbmUtMQ",
        pass: "cGhvbmUtMQ.pass",
        pc: this.pc.fingerprint,
        pcName: this.pc.pcName,
        passkey: {
          challenge: encode(crypto.getRandomValues(new Uint8Array(32))),
          rpId: "localhost",
          user: encode(new Uint8Array(16)),
          userName: "Plenipo on Office PC",
        },
      },
    });
  }

  private async reply(re: string, reply: Omit<PhoneReply, "re">) {
    await this.sendSealed({ t: "reply", reply: { re, ...reply } });
  }

  private refuse(code: string) {
    this.say({ t: "refused", code });
    this.close();
  }
}
