import { describe, expect, it } from "vitest";

import { MAX_LINK_CHARS, hiddenCount, oneLine, pieces, type Piece } from "./safeText";

/** Every character that is always hidden, whatever stands around it: [from, to]. */
const ALWAYS_HIDDEN: readonly (readonly [number, number])[] = [
  [0x0000, 0x0008], // control characters, but not tab (9) or line feed (10)
  [0x000b, 0x001f], // more control characters, with the line return (13)
  [0x007f, 0x009f], // delete, and the controls of the old 8-bit sets
  [0x00ad, 0x00ad], // soft hyphen
  [0x034f, 0x034f],
  [0x061c, 0x061c],
  [0x115f, 0x1160], // Korean fillers
  [0x17b4, 0x17b5],
  [0x180b, 0x180f],
  [0x200b, 0x200b], // zero-width space
  [0x200e, 0x200f], // left-to-right and right-to-left marks
  [0x2028, 0x2029], // line and paragraph separators
  [0x202a, 0x202e], // the marks that turn text, with the right-to-left override
  [0x2060, 0x206f],
  [0x3164, 0x3164],
  [0xfe00, 0xfe0d], // style selectors (the last two are in CONTEXT_HIDDEN)
  [0xfeff, 0xfeff],
  [0xffa0, 0xffa0],
  [0xfff9, 0xfffb],
  [0xe0000, 0xe001f], // tag characters (the rest are in CONTEXT_HIDDEN)
  [0xe0100, 0xe01ef],
  [0xd800, 0xdfff], // halves of a pair, with no other half
];

/** Hidden, except inside an emoji or a word: joiners, style selectors, tags. */
const CONTEXT_HIDDEN: readonly (readonly [number, number])[] = [
  [0x200c, 0x200d],
  [0xfe0e, 0xfe0f],
  [0xe0020, 0xe007f],
];

const inRanges = (code: number, ranges: readonly (readonly [number, number])[]) =>
  ranges.some(([from, to]) => code >= from && code <= to);

/** "U+202E": the code of a character, as Unicode writes it. */
const label = (code: number) => `U+${code.toString(16).toUpperCase().padStart(4, "0")}`;

/** The parts put back together: what a screen would show, in order, hidden marks as they were. */
const rebuild = (parts: readonly Piece[]) =>
  parts.map((part) => (part.kind === "hidden" ? part.char : part.text)).join("");

/**
 * Cut `text` into parts, and check everything that must be true for any text: nothing is lost or
 * moved; text parts are not empty and do not sit side by side; a hidden part is one character
 * with its code and a name; a link is only an address; a character that is always hidden is
 * never inside a text or a link; and `hiddenCount` and `oneLine` agree with the parts.
 */
function cut(text: string): Piece[] {
  const parts = pieces(text);
  expect(rebuild(parts)).toBe(text);
  let before: Piece | undefined;
  for (const part of parts) {
    if (part.kind === "text") {
      expect(part.text).not.toBe("");
      expect(before?.kind).not.toBe("text");
    } else if (part.kind === "hidden") {
      expect([...part.char]).toHaveLength(1);
      expect(part.code).toBe(label(part.char.codePointAt(0) ?? -1));
      expect(part.code).toMatch(/^U\+[0-9A-F]{4,5}$/);
      expect(part.name).not.toBe("");
    } else {
      expect(part.url).toBe(part.text);
      expect(part.text).toMatch(/^https?:\/\/[^\s<>"]+$/i);
      expect([...part.text].length).toBeLessThanOrEqual(MAX_LINK_CHARS);
    }
    if (part.kind !== "hidden") {
      for (const char of part.text) {
        expect(inRanges(char.codePointAt(0) ?? 0, ALWAYS_HIDDEN)).toBe(false);
      }
    }
    before = part;
  }
  expect(hiddenCount(text)).toBe(parts.filter((part) => part.kind === "hidden").length);
  const oneLined = oneLine(text);
  expect([...oneLined]).toHaveLength([...text].length);
  expect(oneLined).not.toMatch(/[\r\n]/);
  return parts;
}

/** The parts in short form: "text:…", "hidden:U+202E", "link:…". */
const brief = (text: string) =>
  cut(text).map((part) =>
    part.kind === "hidden" ? `hidden:${part.code}` : `${part.kind}:${part.text}`,
  );

/** Text that must come out as one text part, as it went in. */
const untouched = (text: string) => expect(cut(text)).toEqual([{ kind: "text", text }]);

/** The addresses offered in `text`. */
const links = (text: string) =>
  cut(text).flatMap((part) => (part.kind === "link" ? [part.url] : []));

describe("plain text", () => {
  it("leaves plain ASCII alone, with tabs and line breaks", () => {
    untouched("Hello, world! 123 (really) - ok?");
    untouched("Line one\nLine two\n\tindented");
    let keyboard = "";
    for (let code = 0x20; code < 0x7f; code++) keyboard += String.fromCharCode(code);
    untouched(keyboard);
  });

  it("makes no parts from no text", () => {
    expect(pieces("")).toEqual([]);
    expect(hiddenCount("")).toBe(0);
    expect(oneLine("")).toBe("");
  });

  it("leaves Arabic and Hebrew alone", () => {
    untouched("مرحبا بالعالم، كيف حالك؟");
    untouched("שלום עולם, מה שלומך?");
    untouched("مرحبا שלום");
  });

  it("leaves text of other languages alone", () => {
    untouched("你好，世界");
    untouched("こんにちは、世界");
    untouched("안녕하세요");
    untouched("Привет, мир");
    untouched("Γειά σου Κόσμε");
    untouched("नमस\u{094D}त\u{0947} द\u{0941}न\u{093F}य\u{093E}");
    untouched("สว\u{0E31}สด\u{0E35}ชาวโลก");
    untouched("Crème brûlée, Ñandú, Åse, İstanbul");
  });

  it("leaves mixed writing directions alone when nothing is hidden", () => {
    untouched("Hello שלום world مرحبا 123 ok");
    untouched("Price: 25 ₪ (שקל) / 7 جنيه");
  });

  it("leaves spaces that are not hidden alone", () => {
    untouched("a\u{00A0}b\u{2003}c\u{3000}d\u{200A}e"); // no-break, em, ideographic, hair spaces
    untouched("A\u{FFFD}B"); // a replacement mark someone typed
    untouched("e\u{0301} and a\u{20E3}"); // marks that join a letter
  });
});

describe("hidden characters", () => {
  it("shows the classic file-name spoof with the override as a visible mark", () => {
    expect(cut("invoice\u{202E}txt.exe")).toEqual([
      { kind: "text", text: "invoice" },
      { kind: "hidden", code: "U+202E", name: "right-to-left override", char: "\u{202E}" },
      { kind: "text", text: "txt.exe" },
    ]);
    expect(hiddenCount("invoice\u{202E}txt.exe")).toBe(1);
  });

  it("makes one hidden part for each hidden character, in its place", () => {
    expect(brief("a\u{200B}\u{200B}b\u{202E}\u{202C}c")).toEqual([
      "text:a",
      "hidden:U+200B",
      "hidden:U+200B",
      "text:b",
      "hidden:U+202E",
      "hidden:U+202C",
      "text:c",
    ]);
    expect(brief("\u{202E}")).toEqual(["hidden:U+202E"]);
    expect(hiddenCount("a\u{200B}\u{200B}b\u{202E}\u{202C}c")).toBe(4);
  });

  it("hides every character of every range, alone and between two letters", () => {
    let checked = 0;
    for (const [from, to] of ALWAYS_HIDDEN) {
      for (let code = from; code <= to; code++) {
        const char = String.fromCodePoint(code);
        expect(brief(char)).toEqual([`hidden:${label(code)}`]);
        expect(brief(`a${char}b`)).toEqual(["text:a", `hidden:${label(code)}`, "text:b"]);
        expect(hiddenCount(`a${char}b`)).toBe(1);
        checked += 1;
      }
    }
    expect(checked).toBe(2441);
  });

  it("hides tab and line feed never, and every other control character always", () => {
    untouched("a\tb\nc");
    for (let code = 0; code <= 0x1f; code++) {
      const char = String.fromCharCode(code);
      if (code === 0x09 || code === 0x0a) {
        untouched(char);
      } else {
        expect(brief(char)).toEqual([`hidden:${label(code)}`]);
      }
    }
    expect(brief("a\u000Db")).toEqual(["text:a", "hidden:U+000D", "text:b"]); // a line return
    expect(brief("a\r\nb")).toEqual(["text:a", "hidden:U+000D", "text:\nb"]);
    expect(brief("a\u007Fb")).toEqual(["text:a", "hidden:U+007F", "text:b"]); // delete
    for (let code = 0x80; code <= 0x9f; code++) {
      expect(brief(String.fromCharCode(code))).toEqual([`hidden:${label(code)}`]);
    }
  });

  it("hides the joiners, the style selectors, and the tags when they stand alone", () => {
    for (const [from, to] of CONTEXT_HIDDEN) {
      for (let code = from; code <= to; code++) {
        expect(brief(String.fromCodePoint(code))).toEqual([`hidden:${label(code)}`]);
      }
    }
  });

  it("writes the code in capitals, with four digits, and five for the tag planes", () => {
    expect(cut("\u0000")).toEqual([expect.objectContaining({ code: "U+0000" })]);
    expect(cut("\u000B")).toEqual([expect.objectContaining({ code: "U+000B" })]);
    expect(cut("\u009F")).toEqual([expect.objectContaining({ code: "U+009F" })]);
    expect(cut("\u{00AD}")).toEqual([expect.objectContaining({ code: "U+00AD" })]);
    expect(cut("\u{FEFF}")).toEqual([expect.objectContaining({ code: "U+FEFF" })]);
    expect(cut("\u{E0001}")).toEqual([expect.objectContaining({ code: "U+E0001" })]);
    expect(cut("\u{E01EF}")).toEqual([expect.objectContaining({ code: "U+E01EF" })]);
    expect(cut("\u{E0000}")).toEqual([expect.objectContaining({ code: "U+E0000" })]);
  });

  it("names each kind of hidden character in plain words", () => {
    const names: readonly (readonly [number, string])[] = [
      [0x0000, "control character"],
      [0x001b, "control character"],
      [0x000d, "line return"],
      [0x007f, "control character"],
      [0x0085, "control character"],
      [0x00ad, "soft hyphen"],
      [0x200b, "zero-width space"],
      [0x200c, "zero-width non-joiner"],
      [0x200d, "zero-width joiner"],
      [0x200e, "left-to-right mark"],
      [0x200f, "right-to-left mark"],
      [0x2028, "line separator"],
      [0x2029, "paragraph separator"],
      [0x202d, "left-to-right override"],
      [0x202e, "right-to-left override"],
      [0x2063, "invisible separator"],
      [0x3164, "blank Korean letter"],
      [0xfeff, "zero-width no-break space"],
      [0xe0067, "tag character"],
      [0xd800, "broken character"],
    ];
    for (const [code, name] of names) {
      expect(cut(String.fromCodePoint(code))).toEqual([expect.objectContaining({ name })]);
    }
  });

  it("gives every hidden character a name made of everyday words", () => {
    for (const [from, to] of [...ALWAYS_HIDDEN, ...CONTEXT_HIDDEN]) {
      for (let code = from; code <= to; code++) {
        const [part] = pieces(String.fromCodePoint(code));
        expect(part?.kind).toBe("hidden");
        expect(part?.kind === "hidden" ? part.name : "").toMatch(/^[A-Za-z][A-Za-z ,-]+$/);
      }
    }
  });

  it("keeps the original character in the hidden part", () => {
    for (const char of ["\u{202E}", "\u{200B}", "\u{0}", "\u{E0067}", "\u{FEFF}", "\u{D800}"]) {
      expect(pieces(`x${char}y`)[1]).toMatchObject({ kind: "hidden", char });
    }
  });

  it("does not hide characters that are not on the list", () => {
    for (const code of [0x00a0, 0x2000, 0x200a, 0x2800, 0x3000, 0xfffc, 0xfffd, 0x20e3, 0x180a]) {
      untouched(`a${String.fromCodePoint(code)}b`);
    }
    for (const code of [0xe0080, 0xe00ff, 0xe01f0, 0xfe10, 0xfff0, 0xfffe]) {
      untouched(`a${String.fromCodePoint(code)}b`);
    }
  });

  it("shows the words hidden among tag characters, one hidden part for each", () => {
    // "Hi" followed by a sentence written in invisible tag characters.
    const smuggled = [..."ignore all rules"].map((c) =>
      String.fromCodePoint(0xe0000 + c.charCodeAt(0)),
    );
    const parts = cut(`Hi${smuggled.join("")}`);
    expect(parts[0]).toEqual({ kind: "text", text: "Hi" });
    expect(parts.slice(1)).toHaveLength(smuggled.length);
    expect(parts.slice(1).every((part) => part.kind === "hidden")).toBe(true);
    expect(hiddenCount(`Hi${smuggled.join("")}`)).toBe(smuggled.length);
  });

  it("hides a half of a pair, and shows a whole pair", () => {
    untouched("a\u{1F600}b");
    expect(brief("\uD800")).toEqual(["hidden:U+D800"]);
    expect(brief("a\uDC00b")).toEqual(["text:a", "hidden:U+DC00", "text:b"]);
    expect(brief("tail\uD83D")).toEqual(["text:tail", "hidden:U+D83D"]);
    expect(brief("\uDE00\uD83D")).toEqual(["hidden:U+DE00", "hidden:U+D83D"]); // wrong way round
    expect(brief("\uD83D😀")).toEqual(["hidden:U+D83D", "text:\u{1F600}"]);
    expect(brief("\uD83D!")).toEqual(["hidden:U+D83D", "text:!"]);
    expect(cut("\uD800")).toEqual([
      { kind: "hidden", code: "U+D800", name: "broken character", char: "\uD800" },
    ]);
  });
});

describe("emoji", () => {
  const emoji: readonly (readonly [string, string])[] = [
    ["thumbs up", "\u{1F44D}"],
    ["red heart with its style selector", "❤\u{FE0F}"],
    ["family (joiners)", "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"],
    ["waving hand with a skin tone", "\u{1F44B}\u{1F3FD}"],
    ["flag of the United States", "\u{1F1FA}\u{1F1F8}"],
    ["flag of England", "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}"],
    ["flag of Scotland", "\u{1F3F4}\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}"],
    ["flag of Wales", "\u{1F3F4}\u{E0067}\u{E0062}\u{E0077}\u{E006C}\u{E0073}\u{E007F}"],
    ["keycap 1", "1\u{FE0F}\u{20E3}"],
    ["keycap #", "#\u{FE0F}\u{20E3}"],
    ["keycap *", "*\u{FE0F}\u{20E3}"],
    ["keycap 0", "0\u{FE0F}\u{20E3}"],
    ["keycap with no style selector", "7\u{20E3}"],
    ["heart on fire (a selector, then a joiner)", "❤\u{FE0F}\u{200D}\u{1F525}"],
    ["rainbow flag", "\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}"],
    ["eye in a speech bubble", "\u{1F441}\u{FE0F}\u{200D}\u{1F5E8}\u{FE0F}"],
    ["woman at a computer, with a skin tone", "\u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}"],
    ["pirate flag", "\u{1F3F4}\u{200D}☠\u{FE0F}"],
    ["man running", "\u{1F3C3}\u{200D}♂\u{FE0F}"],
    ["couple with a heart", "\u{1F469}\u{200D}❤\u{FE0F}\u{200D}\u{1F468}"],
    ["arrow in text style", "↔\u{FE0E}"],
    ["grinning face", "\u{1F600}"],
    ["two flags side by side", "\u{1F1E9}\u{1F1EA}\u{1F1EB}\u{1F1F7}"],
    ["a skin tone, then the next emoji", "\u{1F44D}\u{1F3FF}\u{1F44D}\u{1F3FB}"],
  ];

  it.each(emoji)("leaves %s alone", (_name, text) => {
    untouched(text);
    expect(hiddenCount(text)).toBe(0);
    expect(oneLine(text)).toBe(text);
  });

  it("leaves emoji alone in the middle of sentences, in any direction", () => {
    for (const [, text] of emoji) {
      untouched(`I ${text} you! ${text}${text} (${text})`);
      untouched(`مرحبا ${text} שלום`);
      untouched(`${text}${text}`);
    }
  });

  it("keeps the literal emoji of the examples", () => {
    untouched(
      "👍 ❤\u{FE0F} 👨\u{200D}👩\u{200D}👧 👋🏽 🇺🇸 🏴\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F} 1\u{FE0F}\u{20E3}",
    );
    expect(
      hiddenCount(
        "👍 ❤\u{FE0F} 👨\u{200D}👩\u{200D}👧 👋🏽 🇺🇸 🏴\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F} 1\u{FE0F}\u{20E3}",
      ),
    ).toBe(0);
  });

  it("hides a zero-width joiner between two letters of the Latin alphabet", () => {
    expect(cut("a\u{200D}b")).toEqual([
      { kind: "text", text: "a" },
      { kind: "hidden", code: "U+200D", name: "zero-width joiner", char: "\u{200D}" },
      { kind: "text", text: "b" },
    ]);
    expect(brief("hello\u{200D}world")).toEqual(["text:hello", "hidden:U+200D", "text:world"]);
    expect(brief("e\u{0301}\u{200D}x")).toEqual(["text:e\u{0301}", "hidden:U+200D", "text:x"]);
    expect(brief("é\u{200D}è")).toEqual(["text:é", "hidden:U+200D", "text:è"]);
  });

  it("hides a zero-width joiner anywhere else that is not between two emoji", () => {
    const thumb = "\u{1F44D}";
    expect(brief("\u{200D}")).toEqual(["hidden:U+200D"]);
    expect(brief(`${thumb}\u{200D}`)).toEqual([`text:${thumb}`, "hidden:U+200D"]); // at the end
    expect(brief(`\u{200D}${thumb}`)).toEqual(["hidden:U+200D", `text:${thumb}`]); // at the start
    expect(brief(`${thumb}\u{200D}a`)).toEqual([`text:${thumb}`, "hidden:U+200D", "text:a"]);
    expect(brief(`a\u{200D}${thumb}`)).toEqual(["text:a", "hidden:U+200D", `text:${thumb}`]);
    expect(brief(`${thumb}\u{200D}\u{200D}${thumb}`)).toEqual([
      `text:${thumb}`,
      "hidden:U+200D",
      "hidden:U+200D",
      `text:${thumb}`,
    ]);
    expect(brief("1\u{200D}2")).toEqual(["text:1", "hidden:U+200D", "text:2"]);
    expect(brief(`${thumb} \u{200D}${thumb}`)).toEqual([
      `text:${thumb} `,
      "hidden:U+200D",
      `text:${thumb}`,
    ]);
    expect(brief(`${thumb}\u{200D}\u{202E}${thumb}`)).toEqual([
      `text:${thumb}`,
      "hidden:U+200D",
      "hidden:U+202E",
      `text:${thumb}`,
    ]);
    // A joiner after a style selector that is itself hidden joins nothing.
    expect(brief("x \u{FE0F}\u{200D}\u{1F525}")).toEqual([
      "text:x ",
      "hidden:U+FE0F",
      "hidden:U+200D",
      "text:\u{1F525}",
    ]);
  });

  it("hides a style selector that does not follow a visible character", () => {
    expect(brief("\u{FE0F}")).toEqual(["hidden:U+FE0F"]);
    expect(brief("\u{FE0E}x")).toEqual(["hidden:U+FE0E", "text:x"]);
    expect(brief("a \u{FE0F}b")).toEqual(["text:a ", "hidden:U+FE0F", "text:b"]);
    expect(brief("a\n\u{FE0F}")).toEqual(["text:a\n", "hidden:U+FE0F"]);
    expect(brief("a\tb\t\u{FE0F}")).toEqual(["text:a\tb\t", "hidden:U+FE0F"]);
    expect(brief("\u{202E}\u{FE0F}")).toEqual(["hidden:U+202E", "hidden:U+FE0F"]);
    expect(brief("\u{200B}\u{FE0F}")).toEqual(["hidden:U+200B", "hidden:U+FE0F"]);
  });

  it("shows a style selector right after a visible character, and one only", () => {
    untouched("A\u{FE0F}");
    untouched("A\u{FE0E}");
    untouched("é\u{FE0F}");
    expect(brief("A\u{FE0F}\u{FE0F}")).toEqual(["text:A\u{FE0F}", "hidden:U+FE0F"]);
    expect(brief("A\u{FE0F}\u{FE0E}")).toEqual(["text:A\u{FE0F}", "hidden:U+FE0E"]);
  });

  it("always hides the other style selectors", () => {
    expect(brief("A\u{FE00}")).toEqual(["text:A", "hidden:U+FE00"]);
    expect(brief("A\u{FE0D}")).toEqual(["text:A", "hidden:U+FE0D"]);
    expect(brief("A\u{E0100}")).toEqual(["text:A", "hidden:U+E0100"]);
    expect(brief("\u{1F600}\u{E01EF}")).toEqual(["text:\u{1F600}", "hidden:U+E01EF"]);
  });

  it("shows tag characters only on a flag, and only a few", () => {
    const flag = "\u{1F3F4}";
    const tag = (letter: string) => String.fromCodePoint(0xe0000 + letter.charCodeAt(0));
    const cancel = "\u{E007F}";
    // After something that is not the flag, or after a flag with something between.
    expect(brief(`a${tag("g")}`)).toEqual(["text:a", "hidden:U+E0067"]);
    expect(brief(`${flag}x${tag("g")}`)).toEqual([`text:${flag}x`, "hidden:U+E0067"]);
    expect(brief(`${flag}\u{FE0F}${tag("g")}`)).toEqual([`text:${flag}\u{FE0F}`, "hidden:U+E0067"]);
    // A cancel tag with no tags before it, and after the end of a row.
    expect(brief(cancel)).toEqual(["hidden:U+E007F"]);
    expect(brief(`${flag}${cancel}`)).toEqual([`text:${flag}`, "hidden:U+E007F"]);
    const england = `${flag}${tag("g")}${tag("b")}${tag("e")}${tag("n")}${tag("g")}${cancel}`;
    untouched(england);
    expect(brief(`${england}${tag("g")}`)).toEqual([`text:${england}`, "hidden:U+E0067"]);
    expect(brief(`${england}${cancel}`)).toEqual([`text:${england}`, "hidden:U+E007F"]);
    // The tag space and the language tag are not letters of a flag's name.
    expect(brief(`${flag}\u{E0001}`)).toEqual([`text:${flag}`, "hidden:U+E0001"]);
    expect(brief(`${flag}\u{E0020}`)).toEqual([`text:${flag}\u{E0020}`]);
    // A row of tags on a flag is cut at eight: a flag cannot carry a hidden sentence.
    const longRow = `${flag}${tag("a").repeat(9)}${cancel}`;
    expect(brief(longRow)).toEqual([
      `text:${flag}${tag("a").repeat(8)}`,
      "hidden:U+E0061",
      "hidden:U+E007F",
    ]);
    // Two flags in a row each have their own tags.
    untouched(`${england}${england}`);
  });
});

describe("joiners in words", () => {
  it("leaves the zero-width non-joiner in Persian words", () => {
    untouched("می\u{200C}خواهم");
    untouched("کتاب\u{200C}ها");
    untouched("می\u{200C}خواهم کتاب\u{200C}ها را");
    expect(hiddenCount("می\u{200C}خواهم")).toBe(0);
  });

  it("leaves the joiners in words of the Indian writing systems", () => {
    untouched("क\u{094D}\u{200D}ष"); // a joiner after a virama
    untouched("क\u{094D}\u{200C}ष"); // the same, with a non-joiner
    untouched("ශ\u{0DCA}\u{200D}ර\u{0DD3}"); // Sinhala
    untouched("م\u{200D}م"); // Arabic: a joiner between two letters
  });

  it("hides a zero-width non-joiner that is not between two letters or marks", () => {
    const persian = "م";
    expect(brief("\u{200C}")).toEqual(["hidden:U+200C"]);
    expect(brief(`${persian}\u{200C}`)).toEqual([`text:${persian}`, "hidden:U+200C"]);
    expect(brief(`\u{200C}${persian}`)).toEqual(["hidden:U+200C", `text:${persian}`]);
    expect(brief(`${persian} \u{200C}${persian}`)).toEqual([
      `text:${persian} `,
      "hidden:U+200C",
      `text:${persian}`,
    ]);
    expect(brief(`${persian}\u{200C}1`)).toEqual([`text:${persian}`, "hidden:U+200C", "text:1"]);
    expect(brief(`${persian}\u{200C}\u{200C}${persian}`)).toEqual([
      `text:${persian}`,
      "hidden:U+200C",
      "hidden:U+200C",
      `text:${persian}`,
    ]);
    expect(brief(`${persian}\u{200B}\u{200C}${persian}`)).toEqual([
      `text:${persian}`,
      "hidden:U+200B",
      "hidden:U+200C",
      `text:${persian}`,
    ]);
    // A blank Korean letter is hidden, so it does not make a word.
    expect(brief(`\u{3164}\u{200C}${persian}`)).toEqual([
      "hidden:U+3164",
      "hidden:U+200C",
      `text:${persian}`,
    ]);
  });

  it("hides a zero-width non-joiner in a word of the Latin alphabet", () => {
    expect(brief("a\u{200C}b")).toEqual(["text:a", "hidden:U+200C", "text:b"]);
    expect(brief("م\u{200C}b")).toEqual(["text:م", "hidden:U+200C", "text:b"]);
    expect(brief("b\u{200C}م")).toEqual(["text:b", "hidden:U+200C", "text:م"]);
  });
});

describe("links", () => {
  it("finds an address in the middle of text", () => {
    expect(cut("Visit https://example.com/page for details")).toEqual([
      { kind: "text", text: "Visit " },
      { kind: "link", text: "https://example.com/page", url: "https://example.com/page" },
      { kind: "text", text: " for details" },
    ]);
  });

  it("finds an address at the start, at the end, and on its own", () => {
    expect(brief("https://example.com")).toEqual(["link:https://example.com"]);
    expect(brief("go https://example.com")).toEqual(["text:go ", "link:https://example.com"]);
    expect(brief("https://example.com now")).toEqual(["link:https://example.com", "text: now"]);
  });

  it("finds http and https, and any capital letters in the scheme", () => {
    expect(links("http://example.com/a")).toEqual(["http://example.com/a"]);
    expect(links("https://example.com/a")).toEqual(["https://example.com/a"]);
    expect(links("HTTPS://EXAMPLE.COM/Path")).toEqual(["HTTPS://EXAMPLE.COM/Path"]);
    expect(links("HtTp://example.com/a")).toEqual(["HtTp://example.com/a"]);
    expect(links("Http://example.com")).toEqual(["Http://example.com"]);
  });

  it("offers the address exactly as it was written", () => {
    const address = "HTTPS://Example.COM:8080/a/b%20c?x=1&y=2#top";
    expect(cut(`see ${address} ok`)[1]).toEqual({ kind: "link", text: address, url: address });
  });

  it("finds several addresses, on one line and on several", () => {
    expect(
      links("a https://one.example and http://two.example/x, then https://three.example."),
    ).toEqual(["https://one.example", "http://two.example/x", "https://three.example"]);
    expect(links("https://one.example\nhttps://two.example\thttps://three.example")).toHaveLength(
      3,
    );
  });

  it("leaves the end of a sentence out of the address", () => {
    expect(brief("See https://example.com.")).toEqual([
      "text:See ",
      "link:https://example.com",
      "text:.",
    ]);
    expect(links("Is it https://example.com/a?")).toEqual(["https://example.com/a"]);
    expect(links("a https://example.com/a, b")).toEqual(["https://example.com/a"]);
    expect(links("a https://example.com/a; b")).toEqual(["https://example.com/a"]);
    expect(links("a https://example.com/a: b")).toEqual(["https://example.com/a"]);
    expect(links("wow https://example.com/a!!!")).toEqual(["https://example.com/a"]);
    expect(links("https://example.com/path.,;:!?")).toEqual(["https://example.com/path"]);
    expect(brief("https://example.com/path.,;:!?")).toEqual([
      "link:https://example.com/path",
      "text:.,;:!?",
    ]);
    expect(links("https://example.com?")).toEqual(["https://example.com"]);
    expect(links("https://example.com:?")).toEqual(["https://example.com"]);
    expect(links("https://example.com/a]}")).toEqual(["https://example.com/a"]);
    expect(links("'https://example.com/a'")).toEqual(["https://example.com/a"]);
    expect(links("https://example.com/a'.")).toEqual(["https://example.com/a"]);
  });

  it("keeps what is inside the address", () => {
    expect(links("https://example.com/a.b,c;d:e!f?g=h#i")).toEqual([
      "https://example.com/a.b,c;d:e!f?g=h#i",
    ]);
    expect(links("https://example.com/redirect?to=a@b.example")).toEqual([
      "https://example.com/redirect?to=a@b.example",
    ]);
    expect(links("https://example.com/@person")).toEqual(["https://example.com/@person"]);
    expect(links("https://example.com/\u{1F44D}")).toEqual(["https://example.com/\u{1F44D}"]);
    expect(links("https://example.com/مرحبا")).toHaveLength(1);
    expect(links("https://example.com/路径")).toHaveLength(1);
  });

  it("finds an address in parentheses and brackets, and ends it at the closing one", () => {
    expect(brief("(https://example.com/a)")).toEqual([
      "text:(",
      "link:https://example.com/a",
      "text:)",
    ]);
    expect(brief("(see https://example.com/a).")).toEqual([
      "text:(see ",
      "link:https://example.com/a",
      "text:).",
    ]);
    expect(brief("[https://example.com/a]")).toEqual([
      "text:[",
      "link:https://example.com/a",
      "text:]",
    ]);
    expect(brief("{https://example.com/a}")).toEqual([
      "text:{",
      "link:https://example.com/a",
      "text:}",
    ]);
    expect(brief("<https://example.com/a>")).toEqual([
      "text:<",
      "link:https://example.com/a",
      "text:>",
    ]);
    expect(brief('"https://example.com/a"')).toEqual([
      'text:"',
      "link:https://example.com/a",
      'text:"',
    ]);
    expect(brief("'https://example.com/a'")).toEqual([
      "text:'",
      "link:https://example.com/a",
      "text:'",
    ]);
  });

  it("keeps a closing parenthesis that has its opening one in the address", () => {
    const rust = "https://en.wikipedia.org/wiki/Rust_(programming_language)";
    expect(links(rust)).toEqual([rust]);
    expect(links(`${rust}.`)).toEqual([rust]);
    expect(links(`${rust}, and more`)).toEqual([rust]);
    expect(brief(`(see ${rust})`)).toEqual(["text:(see ", `link:${rust}`, "text:)"]);
    expect(brief("(see https://example.com/a_(b))")).toEqual([
      "text:(see ",
      "link:https://example.com/a_(b)",
      "text:)",
    ]);
    expect(links("https://example.com/(a)(b)")).toEqual(["https://example.com/(a)(b)"]);
    expect(links("https://example.com/(a))")).toEqual(["https://example.com/(a)"]);
    expect(links("https://example.com/(a))).")).toEqual(["https://example.com/(a)"]);
    expect(links("https://example.com/a))")).toEqual(["https://example.com/a"]);
  });

  it("ends an address at a space, a line break, and the marks < > and double quotes", () => {
    expect(brief("https://example.com/a b")).toEqual(["link:https://example.com/a", "text: b"]);
    expect(brief("https://example.com/a\nb")).toEqual(["link:https://example.com/a", "text:\nb"]);
    expect(brief("https://example.com/a\u{00A0}b")).toEqual([
      "link:https://example.com/a",
      "text:\u{00A0}b",
    ]);
    expect(brief("https://example.com/a\u{3000}b")).toEqual([
      "link:https://example.com/a",
      "text:\u{3000}b",
    ]);
    expect(brief("https://example.com/a<b")).toEqual(["link:https://example.com/a", "text:<b"]);
    expect(brief("https://example.com/a>b")).toEqual(["link:https://example.com/a", "text:>b"]);
    expect(brief('https://example.com/a"b')).toEqual(["link:https://example.com/a", 'text:"b']);
  });

  it("ends an address at a hidden character and shows the mark", () => {
    expect(brief("https://exam\u{200B}ple.com")).toEqual([
      "link:https://exam",
      "hidden:U+200B",
      "text:ple.com",
    ]);
    expect(brief("https://example.com/\u{202E}moc.live/")).toEqual([
      "link:https://example.com/",
      "hidden:U+202E",
      "text:moc.live/",
    ]);
    expect(brief("https://example.com/\u0000x")).toEqual([
      "link:https://example.com/",
      "hidden:U+0000",
      "text:x",
    ]);
  });

  it("does not start an address after a hidden character, or after a letter", () => {
    expect(brief("\u{200B}https://example.com")).toEqual([
      "hidden:U+200B",
      "text:https://example.com",
    ]);
    expect(brief("a\u{2028}https://example.com")).toEqual([
      "text:a",
      "hidden:U+2028",
      "text:https://example.com",
    ]);
    expect(links("xhttps://example.com")).toEqual([]);
    expect(links("hhttps://example.com")).toEqual([]);
    expect(links("a,https://example.com")).toEqual([]);
    expect(links("a/https://example.com")).toEqual([]);
    expect(links("a=https://example.com")).toEqual([]);
    expect(links("请访https://example.com")).toEqual([]);
    expect(links("\u{1F44D}https://example.com")).toEqual([]);
  });

  it("finds an address in the middle of a line in another direction", () => {
    expect(brief("زوروا https://example.com الآن")).toEqual([
      "text:زوروا ",
      "link:https://example.com",
      "text: الآن",
    ]);
    expect(links("בואו https://example.com/דג עכשיו")).toEqual(["https://example.com/דג"]);
  });

  it("never makes a link of other kinds of address", () => {
    for (const text of [
      "javascript:alert(1)",
      "JavaScript:alert(1)",
      "see javascript:alert(document.cookie)",
      "file:///etc/passwd",
      "file://example.com/a",
      "data:text/html,<script>alert(1)</script>",
      "data:text/html;base64,PHNjcmlwdD4=",
      "mailto:someone@example.com",
      "tel:+15551234567",
      "ftp://example.com/a",
      "ssh://example.com",
      "vbscript:msgbox(1)",
      "about:blank",
      "chrome://settings",
      "ms-settings:network",
      "//example.com/a",
      "www.example.com",
      "example.com/a",
      "https:example.com",
      "https:/example.com",
      "https//example.com",
      "https:// example.com",
      "https://",
      "http://",
      "https:///path",
      "https://.com",
      "https://a..com",
      "https://:80/",
    ]) {
      expect(links(text)).toEqual([]);
    }
  });

  it("never makes a link of an address with a user name", () => {
    expect(links("https://user@evil.example")).toEqual([]);
    expect(links("https://user@evil.example/path")).toEqual([]);
    expect(links("https://user:pass@evil.example/")).toEqual([]);
    expect(links("https://example.com@evil.example/")).toEqual([]);
    expect(links("https://@evil.example")).toEqual([]);
    expect(links("https://example.com:80@evil.example/")).toEqual([]);
    expect(links("https://good.example\\@evil.example/")).toEqual([]);
    expect(links("(https://user@evil.example)")).toEqual([]);
    expect(brief("https://user@evil.example")).toEqual(["text:https://user@evil.example"]);
  });

  it("never makes a link of an address whose host has other than ASCII letters", () => {
    expect(links("https://ex\u{0430}mple.com/")).toEqual([]); // a Cyrillic "a"
    expect(links("https://\u{0435}xample.com")).toEqual([]);
    expect(links("https://\u{FF45}xample.com")).toEqual([]); // a wide "e"
    expect(links("https://例え.jp/")).toEqual([]);
    expect(links("https://example.com。")).toEqual([]);
    expect(links("https://example.cöm")).toEqual([]);
    expect(links("https://مثال.إختبار")).toEqual([]);
    expect(brief("https://ex\u{0430}mple.com/")).toEqual(["text:https://ex\u{0430}mple.com/"]);
    // The same word in a path is fine: only the host can pass for another site.
    expect(links("https://example.com/ex\u{0430}mple")).toHaveLength(1);
  });

  it("never makes a link of an address whose host is not letters, digits, dots, and hyphens", () => {
    expect(links("https://[::1]/")).toEqual([]);
    expect(links("https://exa_mple.com")).toEqual([]);
    expect(links("https://exa%6Dple.com")).toEqual([]);
    expect(links("https://exa*mple.com")).toEqual([]);
    expect(links("https://example.com:abc/")).toEqual([]);
    expect(links("https://example.com:123456/")).toEqual([]);
    expect(links("https://example.com\\path")).toEqual([]);
  });

  it("finds addresses with a port, an IP number, a one-word host, or an international host", () => {
    expect(links("https://example.com:8080/x")).toEqual(["https://example.com:8080/x"]);
    expect(links("http://localhost:3000/")).toEqual(["http://localhost:3000/"]);
    expect(links("http://192.168.0.1/admin")).toEqual(["http://192.168.0.1/admin"]);
    expect(links("https://xn--80ak6aa92e.com/")).toEqual(["https://xn--80ak6aa92e.com/"]);
    expect(links("https://sub-domain.example.co.uk/a")).toEqual([
      "https://sub-domain.example.co.uk/a",
    ]);
    expect(links("https://example.com./a")).toEqual(["https://example.com./a"]);
    expect(links("https://example.com?x=1")).toEqual(["https://example.com?x=1"]);
    expect(links("https://example.com#top")).toEqual(["https://example.com#top"]);
  });

  it("counts the length of an address up to MAX_LINK_CHARS", () => {
    expect(MAX_LINK_CHARS).toBe(2000);
    const start = "https://example.com/";
    const longest = start + "a".repeat(MAX_LINK_CHARS - start.length);
    expect(longest).toHaveLength(MAX_LINK_CHARS);
    expect(links(longest)).toEqual([longest]);
    expect(links(`see ${longest}.`)).toEqual([longest]); // the full stop is not counted
    expect(links(`${longest})`)).toEqual([longest]);
    expect(links(`${longest}a`)).toEqual([]);
    expect(links(`see ${longest}a.`)).toEqual([]);
    expect(links(`${longest}a ${longest}`)).toEqual([longest]); // only the short one
    expect(brief(`${longest}a`)).toEqual([`text:${longest}a`]);
  });

  it("counts a character as a person does, so an emoji counts as one", () => {
    const start = "https://example.com/";
    const longest = start + "\u{1F44D}".repeat(MAX_LINK_CHARS - start.length);
    expect(links(longest)).toEqual([longest]);
    expect(links(`${longest}\u{1F44D}`)).toEqual([]);
  });

  it("makes a link and the text around it that put back together are the text", () => {
    for (const text of [
      "(https://example.com/a_(b)).",
      "a https://x.example/ b https://y.example/ c",
      "https://x.example/\u{202E}https://y.example/",
      "\u{1F468}\u{200D}\u{1F469} https://example.com/\u{1F44D}\u{200D}\u{1F44D} \u{1F468}\u{200D}\u{1F469}",
    ]) {
      cut(text);
    }
  });
});

describe("hiddenCount", () => {
  it("counts the hidden parts", () => {
    expect(hiddenCount("")).toBe(0);
    expect(hiddenCount("plain text")).toBe(0);
    expect(hiddenCount("a\u{200B}b\u{202E}c")).toBe(2);
    expect(hiddenCount("\u{202E}\u{202E}\u{202E}")).toBe(3);
    expect(hiddenCount("\uD800 and \uDC00")).toBe(2);
    expect(hiddenCount("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}")).toBe(0);
    expect(hiddenCount("a\u{200D}b")).toBe(1);
    expect(hiddenCount("https://exam\u{200B}ple.com")).toBe(1);
  });

  it("does not count a tab or a line feed", () => {
    expect(hiddenCount("a\tb\nc")).toBe(0);
    expect(hiddenCount("a\r\nb")).toBe(1);
  });
});

describe("oneLine", () => {
  it("leaves a plain line alone", () => {
    expect(oneLine("Anna Lee, Lee & Sons (since 1999)")).toBe("Anna Lee, Lee & Sons (since 1999)");
    expect(oneLine("مرحبا שלום 你好")).toBe("مرحبا שלום 你好");
  });

  it("puts U+FFFD where a hidden character was", () => {
    expect(oneLine("Anna\u{202E} Lee")).toBe("Anna\u{FFFD} Lee");
    expect(oneLine("a\u{200B}b\u{200B}c")).toBe("a\u{FFFD}b\u{FFFD}c");
    expect(oneLine("\u0000\u007F\u0085")).toBe("\u{FFFD}\u{FFFD}\u{FFFD}");
    expect(oneLine("\uD800x")).toBe("\u{FFFD}x");
    expect(oneLine("a\u{2028}b\u{2029}c")).toBe("a\u{FFFD}b\u{FFFD}c");
    expect(oneLine("x\u{E0067}y")).toBe("x\u{FFFD}y");
  });

  it("puts U+FFFD where a line break was, one for each character", () => {
    expect(oneLine("one\ntwo")).toBe("one\u{FFFD}two");
    expect(oneLine("one\r\ntwo")).toBe("one\u{FFFD}\u{FFFD}two");
    expect(oneLine("one\rtwo")).toBe("one\u{FFFD}two");
    expect(oneLine("\n\n")).toBe("\u{FFFD}\u{FFFD}");
  });

  it("keeps emoji and the joiners of words, and a tab", () => {
    const text =
      "Co \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} \u{2764}\u{FE0F} \u{1F3F4}\u{E0067}\u{E007F}";
    expect(oneLine(text)).toBe(text);
    const persian = "\u{645}\u{6CC}\u{200C}\u{62E}\u{648}\u{627}\u{647}\u{645}";
    expect(oneLine(persian)).toBe(persian);
    expect(oneLine("a\tb")).toBe("a\tb");
    expect(oneLine("a\u{200D}b")).toBe("a\u{FFFD}b");
  });

  it("never has a line break, whatever the text", () => {
    const breaks = ["\n", "\r", "\u{2028}", "\u{2029}", "\u{85}", "\u{B}", "\u{C}"];
    const oneLined = oneLine(`a${breaks.join("b")}c`);
    expect(oneLined).toBe("a\u{FFFD}b\u{FFFD}b\u{FFFD}b\u{FFFD}b\u{FFFD}b\u{FFFD}b\u{FFFD}c");
    for (const lineBreak of breaks) expect(oneLined).not.toContain(lineBreak);
  });
});

/** A small random-number maker, so a failing text can be made again (mulberry32). */
function randomNumbers(seed: number): () => number {
  let state = seed;
  return () => {
    state = (state + 0x6d2b79f5) | 0;
    let mixed = Math.imul(state ^ (state >>> 15), 1 | state);
    mixed = (mixed + Math.imul(mixed ^ (mixed >>> 7), 61 | mixed)) ^ mixed;
    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4294967296;
  };
}

describe("any text", () => {
  /** Bits and pieces of real text, hidden characters, and the edges of addresses. */
  const BITS: readonly string[] = [
    // ASCII, spaces, and marks that matter to addresses
    ...[
      "a",
      "Z",
      "k",
      "0",
      "9",
      " ",
      "  ",
      "\n",
      "\t",
      ".",
      ",",
      ";",
      ":",
      "!",
      "?",
      "(",
      ")",
      "[",
      "]",
    ],
    ...["{", "}", "<", ">", '"', "'", "/", "\\", "@", "#", "%", "-", "_", "=", "&", "*", "~"],
    ...[
      "hello",
      "world",
      "invoice",
      "exe.txt",
      "http://",
      "https://",
      "HTTPS://",
      "https://example.com",
    ],
    ...[
      "https://example.com/path",
      "example.com",
      "www.example.com",
      "javascript:alert(1)",
      "user@host",
    ],
    // Arabic, Hebrew, Persian, Chinese, Japanese, Korean, Hindi, Thai, Russian, Greek
    ...["مرحبا", "שלום", "می\u{200C}خواهم"],
    ...["你好", "こんにちは", "안녕", "नमस\u{094D}त\u{0947}"],
    ...["क\u{094D}\u{200D}ष", "สว\u{0E31}สด\u{0E35}", "Привет"],
    // Emoji of all kinds
    ...[
      "\u{1F44D}",
      "❤\u{FE0F}",
      "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
      "\u{1F44B}\u{1F3FD}",
    ],
    ...[
      "\u{1F1FA}\u{1F1F8}",
      "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}",
      "1\u{FE0F}\u{20E3}",
    ],
    ...["\u{1F600}", "❤\u{FE0F}\u{200D}\u{1F525}", "\u{1F3F4}", "\u{1F3FD}", "\u{20E3}", "©"],
    // Hidden characters, and the ones that sometimes are
    ...[
      "\u{202E}",
      "\u{202D}",
      "\u{202C}",
      "\u{2066}",
      "\u{2069}",
      "\u{200E}",
      "\u{200F}",
      "\u{061C}",
      "\u{200B}",
    ],
    ...[
      "\u{200C}",
      "\u{200D}",
      "\u{FE0F}",
      "\u{FE0E}",
      "\u{FE00}",
      "\u{FEFF}",
      "\u{00AD}",
      "\u{034F}",
      "\u{2060}",
    ],
    ...[
      "\u{2063}",
      "\u{2028}",
      "\u{2029}",
      "\u0000",
      "\u0008",
      "\u000D",
      "\u001B",
      "\u007F",
      "\u0085",
    ],
    ...[
      "\u009F",
      "\u{115F}",
      "\u{3164}",
      "\u{FFA0}",
      "\u{17B4}",
      "\u{180E}",
      "\u{FFF9}",
      "\u{E0001}",
      "\u{E0020}",
    ],
    ...["\u{E0067}", "\u{E007E}", "\u{E007F}", "\u{E0100}", "\u{E01EF}"],
    // Halves of pairs with no other half
    ...["\uD800", "\uDBFF", "\uDC00", "\uDFFF", "\uD83D", "\uDE00"],
  ];

  /** One random text, put together from the bits above. */
  function fromBits(next: () => number): string {
    let text = "";
    for (let count = Math.floor(next() * 30); count > 0; count--) {
      text += BITS[Math.floor(next() * BITS.length)] ?? "";
    }
    return text;
  }

  /** One random text of single characters, from anywhere in the writing systems of the world. */
  function fromCharacters(next: () => number): string {
    const kinds: readonly (readonly [number, number])[] = [
      [0x00, 0x7f], // ASCII
      [0x80, 0x24f], // the Latin alphabet, with its extras
      [0x590, 0x6ff], // Hebrew and Arabic
      [0x900, 0xdff], // the Indian writing systems
      [0x2000, 0x206f], // spaces, joiners, marks, and the marks that turn text
      [0x3040, 0x30ff], // Japanese
      [0x4e00, 0x4fff], // Chinese
      [0xd800, 0xdfff], // halves of pairs
      [0xfe00, 0xfeff], // selectors and the byte order mark
      [0x1f300, 0x1f64f], // pictures
      [0xe0000, 0xe01ef], // tags and selectors
    ];
    let text = "";
    for (let count = Math.floor(next() * 40); count > 0; count--) {
      const [from, to] = kinds[Math.floor(next() * kinds.length)] ?? [0, 0x7f];
      text += String.fromCodePoint(from + Math.floor(next() * (to - from + 1)));
    }
    return text;
  }

  it("gives back the text exactly: 3000 random texts of bits of real text", () => {
    const next = randomNumbers(20261003);
    for (let round = 0; round < 3000; round++) cut(fromBits(next));
  });

  it("gives back the text exactly: 3000 random texts of single characters", () => {
    const next = randomNumbers(8);
    for (let round = 0; round < 3000; round++) cut(fromCharacters(next));
  });

  it("gives back the text exactly, for a list of hard texts", () => {
    for (const text of [
      "",
      " ",
      "a",
      "\n",
      "\u{202E}",
      "invoice\u{202E}txt.exe",
      "\uD800",
      "\u{1F600}\uD800\u{1F600}",
      "https://",
      "https://a",
      "(((https://a.example)))",
      "\u{200D}\u{200D}\u{200D}",
      "\u{1F44D}\u{200D}\u{200D}\u{1F44D}",
      "\u{1F3F4}\u{E0067}",
      "\u{FE0F}\u{FE0F}\u{FE0F}",
      "a\u{FE0F}\u{FE0F}\u{FE0F}",
      "https://a.example/https://b.example/\u{202E}https://c.example",
      "x".repeat(10_000),
    ]) {
      cut(text);
    }
  });

  it("does not take long for a very long text built to be slow", () => {
    const started = Date.now();
    for (const text of [
      "(http://".repeat(40_000) + ")".repeat(40_000),
      "(https://example.com/".repeat(40_000) + ")".repeat(40_000),
      "https://example.com/" + ".".repeat(200_000),
      "https://example.com/a" + ")".repeat(100_000) + " " + "(https://example.com/".repeat(5_000),
      "http://".repeat(100_000),
      "a ".repeat(100_000) + "\u{200D}".repeat(100_000),
      "\u{1F3F4}" + "\u{E0067}".repeat(100_000),
    ]) {
      const parts = pieces(text);
      expect(rebuild(parts)).toBe(text);
      expect(hiddenCount(text)).toBe(parts.filter((part) => part.kind === "hidden").length);
    }
    expect(Date.now() - started).toBeLessThan(10_000);
  });
});
