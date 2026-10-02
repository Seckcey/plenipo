import { decode, encode } from "../lock/bytes";

/**
 * The phone's connection to 8 West's relay (`contracts/phone-relay/v1`): one WebSocket, one JSON
 * object per text message. The relay only passes sealed messages along; it never reads them.
 */

/** Where the page reaches the relay, built in (Plenipo's relay name, or the tests' stand-in). */
export const RELAY: string = __PLENIPO_RELAY__;

/** The relay said no (`pc_offline`, `bad_pass`, `mailbox_closed`, `too_many_tries`…). */
export class RelayRefused extends Error {
  constructor(readonly code: string) {
    super(`the relay said ${code}`);
  }
}

/** The connection ended: the PC went away, or the connection closed. */
export class LineEnded extends Error {
  constructor(readonly why: "pcOffline" | "closed" | "timeout") {
    super(`the connection ended (${why})`);
  }
}

/** What opens a WebSocket (the tests give their own). */
export type SocketMaker = (url: string) => WebSocket;

const MAX_MESSAGE = 96 * 1024;

export type First = { t: "pass"; pass: string } | { t: "mailbox"; mailbox: string };

export class Relay {
  private queue: Uint8Array[] = [];
  private waiters: { resolve: (d: Uint8Array) => void; reject: (e: Error) => void }[] = [];
  private ended: LineEnded | null = null;
  private endListeners: ((e: LineEnded) => void)[] = [];

  private constructor(private readonly socket: WebSocket) {}

  /** Connect, and show the relay the pass or the mailbox. Resolves once the relay is ready. */
  static connect(
    first: First,
    url: string = RELAY,
    make: SocketMaker = (u) => new WebSocket(u),
  ): Promise<Relay> {
    return new Promise((resolve, reject) => {
      let socket: WebSocket;
      try {
        socket = make(url);
      } catch {
        reject(new LineEnded("closed"));
        return;
      }
      const relay = new Relay(socket);
      let ready = false;
      const timer = setTimeout(() => {
        if (!ready) {
          reject(new LineEnded("timeout"));
          socket.close();
        }
      }, 20_000);
      socket.onopen = () => socket.send(JSON.stringify(first));
      socket.onmessage = (event: MessageEvent) => {
        if (typeof event.data !== "string" || event.data.length > MAX_MESSAGE) return;
        let m: { t?: string; code?: string; data?: string };
        try {
          m = JSON.parse(event.data) as typeof m;
        } catch {
          return;
        }
        if (!ready) {
          if (m.t === "ready") {
            ready = true;
            clearTimeout(timer);
            resolve(relay);
          } else if (m.t === "refused" || m.t === "pc_offline") {
            clearTimeout(timer);
            reject(new RelayRefused(m.code ?? "pc_offline"));
            socket.close();
          }
          return;
        }
        if (m.t === "data" && typeof m.data === "string") {
          const bytes = decode(m.data);
          if (bytes) relay.deliver(bytes);
        } else if (m.t === "pc_offline") {
          relay.end(new LineEnded("pcOffline"));
        } else if (m.t === "refused") {
          relay.end(new LineEnded("closed"));
        }
      };
      socket.onclose = () => {
        clearTimeout(timer);
        if (!ready) reject(new LineEnded("closed"));
        relay.end(new LineEnded("closed"));
      };
      socket.onerror = () => undefined;
    });
  }

  private deliver(bytes: Uint8Array) {
    const w = this.waiters.shift();
    if (w) w.resolve(bytes);
    else this.queue.push(bytes);
  }

  private end(e: LineEnded) {
    if (this.ended) return;
    this.ended = e;
    for (const w of this.waiters.splice(0)) w.reject(e);
    for (const l of this.endListeners) l(e);
  }

  /** The next sealed message from the PC. */
  next(timeoutMs = 20_000): Promise<Uint8Array> {
    const queued = this.queue.shift();
    if (queued) return Promise.resolve(queued);
    if (this.ended) return Promise.reject(this.ended);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.waiters = this.waiters.filter((w) => w.resolve !== done);
        reject(new LineEnded("timeout"));
      }, timeoutMs);
      const done = (d: Uint8Array) => {
        clearTimeout(timer);
        resolve(d);
      };
      this.waiters.push({
        resolve: done,
        reject: (e) => {
          clearTimeout(timer);
          reject(e);
        },
      });
    });
  }

  /** Send one sealed message to the PC. */
  send(bytes: Uint8Array): void {
    if (this.ended) throw this.ended;
    this.socket.send(JSON.stringify({ t: "data", data: encode(bytes) }));
  }

  onEnd(listener: (e: LineEnded) => void): void {
    if (this.ended) listener(this.ended);
    else this.endListeners.push(listener);
  }

  get isEnded(): boolean {
    return this.ended !== null;
  }

  close(): void {
    this.end(new LineEnded("closed"));
    try {
      this.socket.close();
    } catch {
      // Already closed.
    }
  }
}
