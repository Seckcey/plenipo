// @vitest-environment node
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { decode, encode, fromHex, fromUtf8, hex } from "./bytes";
import { codeFromHash, mailboxOf, parseCode, pskOf, shownCode } from "./code";
import { Assembler, Handshake, fixedKeyPair, newKeyPair } from "./noise";

/** The written contract, as the PC's code wrote it (`crates/remote/src/contract.rs`). */
function contract<T>(name: string): T {
  const path = fileURLToPath(
    new URL(`../../../../contracts/phone-relay/v1/${name}`, import.meta.url),
  );
  return JSON.parse(readFileSync(path, "utf8")) as T;
}

interface Message {
  from: "phone" | "pc";
  payload: string;
  message: string;
}
interface Meeting {
  prologue: string;
  psk?: string;
  phoneStatic: string;
  phoneStaticPublic: string;
  pcStatic: string;
  pcStaticPublic: string;
  phoneEphemeral: string;
  pcEphemeral: string;
  messages: Message[];
  handshakeHash: string;
  sealed: { from: "phone" | "pc"; plain?: string; plainLength?: number; pieces: string[] }[];
}
const vectors = contract<{ pairing: Meeting; everyday: Meeting }>("noise-vectors.json");
const code = contract<{ code: string; shown: string; mailbox: string; psk: string; link: string }>(
  "pairing-code.json",
);

describe("bytes", () => {
  it("reads and writes base64url strictly", () => {
    const bytes = new Uint8Array([0xfb, 0xff, 0x00, 0x10]);
    expect(encode(bytes)).toBe("-_8AEA");
    expect(decode("-_8AEA")).toEqual(bytes);
    expect(decode("-_8AEA==")).toBeNull();
    expect(decode("+/8AEA")).toBeNull();
    expect(decode("-_8AEB")).toBeNull(); // not the one way to write it
    for (let n = 0; n < 40; n++) {
      const b = crypto.getRandomValues(new Uint8Array(n));
      expect(decode(encode(b))).toEqual(b);
    }
  });
});

describe("pairing codes", () => {
  it("reads typed codes kindly, as the PC does", () => {
    expect(parseCode("7k3q-m9tx-2hfd-r8wb")).toBe(code.code);
    expect(parseCode(" 7K3Q M9TX 2HFD R8WB ")).toBe(code.code);
    expect(parseCode("O0IL-0000-0000-0000")).toBe("0011000000000000");
    for (const bad of ["", "7K3Q-M9TX-2HFD-R8W", "7K3Q-M9TX-2HFD-R8WU", "7K3Q-M9TX-2HFD-R8W!"]) {
      expect(parseCode(bad)).toBeNull();
    }
    expect(shownCode(code.code)).toBe(code.shown);
    expect(codeFromHash(new URL(code.link).hash)).toBe(code.code);
    expect(codeFromHash("#other=1")).toBeNull();
  });

  it("makes the same mailbox name and shared key as the PC", async () => {
    expect(await mailboxOf(code.code)).toBe(code.mailbox);
    expect(hex(await pskOf(code.code))).toBe(code.psk);
  });
});

async function fixed(h: string) {
  return fixedKeyPair(fromHex(h));
}

describe("the lock: the PC's own test answers", () => {
  it("the first meeting, from the phone's side", async () => {
    const v = vectors.pairing;
    expect(hex(await pskOf(code.code))).toBe(v.psk);
    const s = await fixed(v.phoneStatic);
    expect(hex(s.publicKey)).toBe(v.phoneStaticPublic);
    const hs = await Handshake.pairing({
      initiator: true,
      s,
      psk: fromHex(v.psk!),
      e: await fixed(v.phoneEphemeral),
    });
    const [m1, m2, m3] = v.messages;
    expect(hex(await hs.writeMessage(fromHex(m1!.payload)))).toBe(m1!.message);
    expect(hex(await hs.readMessage(fromHex(m2!.message)))).toBe(m2!.payload);
    expect(hex(hs.rs!)).toBe(v.pcStaticPublic);
    expect(hex(await hs.writeMessage(fromHex(m3!.payload)))).toBe(m3!.message);
    expect(hex(hs.handshakeHash)).toBe(v.handshakeHash);
    const line = await hs.open();
    const phoneSays = v.sealed.find((x) => x.from === "phone")!;
    const pieces = await line.seal(fromHex(phoneSays.plain!));
    expect(pieces.map(hex)).toEqual(phoneSays.pieces);
    const pcSays = v.sealed.find((x) => x.from === "pc")!;
    const opened = await line.open(fromHex(pcSays.pieces[0]!));
    expect(opened.more).toBe(false);
    expect(hex(opened.bytes)).toBe(pcSays.plain);
  });

  it("the first meeting, from the PC's side (so the phone's code is right both ways)", async () => {
    const v = vectors.pairing;
    const hs = await Handshake.pairing({
      initiator: false,
      s: await fixed(v.pcStatic),
      psk: fromHex(v.psk!),
      e: await fixed(v.pcEphemeral),
    });
    const [m1, m2, m3] = v.messages;
    expect(hex(await hs.readMessage(fromHex(m1!.message)))).toBe(m1!.payload);
    expect(hex(await hs.writeMessage(fromHex(m2!.payload)))).toBe(m2!.message);
    expect(hex(await hs.readMessage(fromHex(m3!.message)))).toBe(m3!.payload);
    expect(hex(hs.rs!)).toBe(v.phoneStaticPublic);
    expect(hex(hs.handshakeHash)).toBe(v.handshakeHash);
  });

  it("a wrong code fails the first meeting at its third message", async () => {
    const v = vectors.pairing;
    const pc = await Handshake.pairing({
      initiator: false,
      s: await fixed(v.pcStatic),
      psk: fromHex(v.psk!),
    });
    const phone = await Handshake.pairing({
      initiator: true,
      s: await newKeyPair(),
      psk: await pskOf("0000000000000000"),
    });
    await pc.readMessage(await phone.writeMessage(new Uint8Array(0)));
    await phone.readMessage(await pc.writeMessage(new Uint8Array(0)));
    const m3 = await phone.writeMessage(new Uint8Array(0));
    await expect(pc.readMessage(m3)).rejects.toThrow();
  });

  it("an everyday meeting, and a long message in pieces", async () => {
    const v = vectors.everyday;
    const hs = await Handshake.everyday({
      initiator: true,
      s: await fixed(v.phoneStatic),
      rs: fromHex(v.pcStaticPublic),
      prologue: fromHex(v.prologue),
      e: await fixed(v.phoneEphemeral),
    });
    const [m1, m2] = v.messages;
    expect(hex(await hs.writeMessage(fromHex(m1!.payload)))).toBe(m1!.message);
    const welcome = await hs.readMessage(fromHex(m2!.message));
    expect(hex(welcome)).toBe(m2!.payload);
    expect(JSON.parse(fromUtf8(welcome))).toMatchObject({ signedIn: false, pcName: "Office PC" });
    expect(hex(hs.handshakeHash)).toBe(v.handshakeHash);
    const line = await hs.open();
    const phoneSays = v.sealed.find((x) => x.from === "phone")!;
    expect((await line.seal(fromHex(phoneSays.plain!))).map(hex)).toEqual(phoneSays.pieces);
    const pcSays = v.sealed.find((x) => x.from === "pc")!;
    expect(pcSays.pieces).toHaveLength(2);
    const assembler = new Assembler();
    let whole: Uint8Array | null = null;
    for (const p of pcSays.pieces) {
      const { more, bytes } = await line.open(fromHex(p));
      whole = assembler.add(more, bytes);
    }
    expect(whole!.length).toBe(pcSays.plainLength);
    expect(fromUtf8(whole!.slice(0, 27))).toBe("abcdefghijklmnopqrstuvwxyza");
  });

  it("a stranger's key, or a copied or changed message, fails", async () => {
    const v = vectors.everyday;
    // Not the phone the PC knows.
    const stranger = await Handshake.everyday({
      initiator: true,
      s: await newKeyPair(),
      rs: fromHex(v.pcStaticPublic),
      prologue: fromHex(v.prologue),
    });
    const pc = await Handshake.everyday({
      initiator: false,
      s: await fixed(v.pcStatic),
      rs: fromHex(v.phoneStaticPublic),
      prologue: fromHex(v.prologue),
    });
    await expect(pc.readMessage(await stranger.writeMessage(new Uint8Array(0)))).rejects.toThrow();
    // A real pair, then a copied and a changed message.
    const phoneSide = await Handshake.everyday({
      initiator: true,
      s: await fixed(v.phoneStatic),
      rs: fromHex(v.pcStaticPublic),
      prologue: fromHex(v.prologue),
    });
    const pcSide = await Handshake.everyday({
      initiator: false,
      s: await fixed(v.pcStatic),
      rs: fromHex(v.phoneStaticPublic),
      prologue: fromHex(v.prologue),
    });
    await pcSide.readMessage(await phoneSide.writeMessage(new Uint8Array(0)));
    await phoneSide.readMessage(await pcSide.writeMessage(new Uint8Array(0)));
    const phoneLine = await phoneSide.open();
    const pcLine = await pcSide.open();
    const [sealed] = await phoneLine.seal(new Uint8Array([1, 2, 3]));
    expect((await pcLine.open(sealed!)).bytes).toEqual(new Uint8Array([1, 2, 3]));
    await expect(pcLine.open(sealed!)).rejects.toThrow();
    const [next] = await phoneLine.seal(new Uint8Array([4]));
    const changed = next!.slice();
    changed[0]! ^= 1;
    await expect(pcLine.open(changed)).rejects.toThrow();
  });

  it("the phone's own key can never be copied out", async () => {
    const pair = await newKeyPair(false);
    expect(pair.privateKey.extractable).toBe(false);
    await expect(crypto.subtle.exportKey("pkcs8", pair.privateKey)).rejects.toThrow();
  });
});
