/**
 * Other people's words, shown safely (Phase 24, ADR-164 §4: private messages sealed). A message
 * holds every letter, number, symbol, and emoji, in any language and any writing direction. The
 * same goes for another person's name, message, company, and line about their business. Those
 * words are not yours and not trusted, so Plenipo never puts them in a page as they are.
 *
 * `pieces` cuts the text into parts a screen can show one by one:
 *
 * - **text**: shown as plain text, never as a web page;
 * - **hidden**: one character a person cannot see but that can disguise words (a mark that turns
 *   the text right to left, a zero-width space, a control character). It is shown as a visible
 *   mark with its name, so nothing hides;
 * - **link**: a web address (`https://` or `http://`). It is shown as text. Opening it asks
 *   "Open this link in your web browser?" first.
 *
 * Nothing is dropped, changed, or moved: put every part back together and you have the original
 * text. Emoji keep working: the invisible characters that glue an emoji together are not hidden
 * when they sit inside one. The Rust side has its own, shorter list of hidden characters
 * (`refuse_hidden_characters` in crates/capabilities/src/desktop.rs) for words a worker types.
 */

/** One part of a text. */
export type Piece =
  /** Plain text to show as it is. */
  | { kind: "text"; text: string }
  /** One hidden character: `code` such as "U+202E", `name` in plain words, `char` as it was. */
  | { kind: "hidden"; code: string; name: string; char: string }
  /** A web address. `url` is the exact address to offer to open (the same as `text`). */
  | { kind: "link"; text: string; url: string };

/** The longest web address Plenipo offers to open, counted in characters. */
export const MAX_LINK_CHARS = 2000;

/** The most tag characters Plenipo shows after a flag. England, Scotland, and Wales use five. */
const MAX_FLAG_TAGS = 8;

const ZERO_WIDTH_NON_JOINER = 0x200c;
const ZERO_WIDTH_JOINER = 0x200d;
const TEXT_STYLE = 0xfe0e;
const EMOJI_STYLE = 0xfe0f;
const BLACK_FLAG = 0x1f3f4;
const FIRST_TAG = 0xe0020;
const LAST_TAG = 0xe007e;
const CANCEL_TAG = 0xe007f;

/** What a person sees for a character that is only half of a pair. */
const REPLACEMENT = "\u{FFFD}";

/** The name of a lone surrogate (half of a pair, which cannot be shown on its own). */
const BROKEN = "broken character";

/**
 * Every character that is hidden (rule 2), lowest first. A few of them stay as text inside an
 * emoji or a word (see `mark`): the joiners, the style selectors, and the tags.
 */
const HIDDEN: readonly (readonly [from: number, to: number, name: string])[] = [
  [0x0000, 0x0008, "control character"],
  [0x000b, 0x000c, "control character"],
  [0x000d, 0x000d, "line return"],
  [0x000e, 0x001f, "control character"],
  [0x007f, 0x009f, "control character"],
  [0x00ad, 0x00ad, "soft hyphen"],
  [0x034f, 0x034f, "invisible joining mark"],
  [0x061c, 0x061c, "Arabic direction mark"],
  [0x115f, 0x1160, "blank Korean letter"],
  [0x17b4, 0x17b5, "invisible Khmer letter"],
  [0x180b, 0x180f, "invisible Mongolian mark"],
  [0x200b, 0x200b, "zero-width space"],
  [0x200c, 0x200c, "zero-width non-joiner"],
  [0x200d, 0x200d, "zero-width joiner"],
  [0x200e, 0x200e, "left-to-right mark"],
  [0x200f, 0x200f, "right-to-left mark"],
  [0x2028, 0x2028, "line separator"],
  [0x2029, 0x2029, "paragraph separator"],
  [0x202a, 0x202a, "start of left-to-right text"],
  [0x202b, 0x202b, "start of right-to-left text"],
  [0x202c, 0x202c, "end of direction change"],
  [0x202d, 0x202d, "left-to-right override"],
  [0x202e, 0x202e, "right-to-left override"],
  [0x2060, 0x2060, "word joiner"],
  [0x2061, 0x2061, "invisible function sign"],
  [0x2062, 0x2062, "invisible times sign"],
  [0x2063, 0x2063, "invisible separator"],
  [0x2064, 0x2064, "invisible plus sign"],
  [0x2065, 0x2065, "unused invisible character"],
  [0x2066, 0x2066, "start of separate left-to-right text"],
  [0x2067, 0x2067, "start of separate right-to-left text"],
  [0x2068, 0x2068, "start of separate text"],
  [0x2069, 0x2069, "end of separate text"],
  [0x206a, 0x206f, "old direction or shape control"],
  [0x3164, 0x3164, "blank Korean letter"],
  [0xfe00, 0xfe0f, "invisible style selector"],
  [0xfeff, 0xfeff, "zero-width no-break space"],
  [0xffa0, 0xffa0, "blank Korean letter"],
  [0xfff9, 0xfffb, "hidden note mark"],
  [0xe0000, 0xe007f, "tag character"],
  [0xe0100, 0xe01ef, "invisible style selector"],
];

/** The name of a hidden character, or null when the character is not on the list. */
function hiddenName(code: number): string | null {
  if (code >= 0x20 && code < 0x7f) return null; // plain keyboard characters: the usual case
  // A code point in this range can only be a half of a pair that has no other half.
  if (code >= 0xd800 && code <= 0xdfff) return BROKEN;
  for (const [from, to, name] of HIDDEN) {
    if (code < from) return null;
    if (code <= to) return name;
  }
  return null;
}

const SPACE = /\s/u;
const LETTER_OR_MARK = /[\p{L}\p{M}]/u;
const LATIN = /\p{Script=Latin}/u;
const EMOJI_PART = /[\p{Extended_Pictographic}\p{Emoji_Modifier}\p{Regional_Indicator}\u{20E3}]/u;

/** Text cut into characters (a pair of halves is one), and which of them are hidden. */
interface Marked {
  chars: string[];
  /** For each character: its name when it is hidden, or null when it is shown. */
  hidden: (string | null)[];
}

/**
 * Decide, for each character, whether it is hidden. Most are decided by the list alone. Five
 * kinds depend on their neighbors, because real writing needs them:
 *
 * - the zero-width joiner stays between two emoji (a family, a person with a job), and between
 *   two letters or marks of a script that uses it (Indic words);
 * - the zero-width non-joiner stays between two letters or marks (Persian, Indic). In both cases
 *   a word in the Latin alphabet gets no joiner: there it can only disguise, so it is hidden;
 * - the style selectors (U+FE0E and U+FE0F) stay right after a visible character;
 * - the tag characters stay after a waving black flag (England, Scotland, Wales), at most
 *   `MAX_FLAG_TAGS` of them, so a flag cannot carry a hidden sentence;
 * - the cancel tag stays when it ends such a row of tags.
 */
function mark(text: string): Marked {
  const chars = Array.from(text);
  const codes = chars.map((char) => char.codePointAt(0) ?? 0);
  const always = codes.map(hiddenName);
  const hidden: (string | null)[] = [];

  // A shown letter or mark. (Hidden ones, such as a blank Korean letter, do not count.)
  const letterOrMark = (at: number) => always[at] === null && LETTER_OR_MARK.test(chars[at] ?? "");
  const latin = (at: number) => LATIN.test(chars[at] ?? "");
  const joinsLetters = (at: number) =>
    letterOrMark(at - 1) && letterOrMark(at + 1) && !latin(at - 1) && !latin(at + 1);
  // A shown character that is not a space: something a style selector can belong to.
  const visible = (at: number) => always[at] === null && !SPACE.test(chars[at] ?? " ");
  const joinsEmoji = (at: number) => {
    let before = at - 1;
    // A style selector sits between an emoji and the joiner (a red heart on fire).
    const selector = codes[before];
    if (hidden[before] === null && (selector === TEXT_STYLE || selector === EMOJI_STYLE)) {
      before -= 1;
    }
    return EMOJI_PART.test(chars[before] ?? "") && EMOJI_PART.test(chars[at + 1] ?? "");
  };

  // After a flag: 0. After each tag shown: one more. Anywhere else: -1.
  let tags = -1;
  for (let at = 0; at < chars.length; at++) {
    const code = codes[at] ?? 0;
    let name = always[at] ?? null;
    let nextTags = code === BLACK_FLAG ? 0 : -1;
    if (name !== null) {
      if (code === ZERO_WIDTH_NON_JOINER) {
        if (joinsLetters(at)) name = null;
      } else if (code === ZERO_WIDTH_JOINER) {
        if (joinsLetters(at) || joinsEmoji(at)) name = null;
      } else if (code === TEXT_STYLE || code === EMOJI_STYLE) {
        if (visible(at - 1)) name = null;
      } else if (code >= FIRST_TAG && code <= LAST_TAG) {
        if (tags >= 0 && tags < MAX_FLAG_TAGS) {
          name = null;
          nextTags = tags + 1;
        }
      } else if (code === CANCEL_TAG) {
        if (tags > 0) name = null;
      }
    }
    hidden.push(name);
    tags = nextTags;
  }
  return { chars, hidden };
}

/** "U+202E": the code of a character, as Unicode writes it (at least four digits). */
function codeOf(char: string): string {
  return `U+${(char.codePointAt(0) ?? 0).toString(16).toUpperCase().padStart(4, "0")}`;
}

/** A stretch of the characters: from `start`, up to but not including `end`. */
interface Span {
  start: number;
  end: number;
}

/** What may stand right before a web address, besides a space or the start of the text. */
const BEFORE_LINK = new Set(["(", "[", "{", "<", '"', "'"]);
/** Where an address ends: a space or a hidden character (checked apart), and these. */
const LINK_STOPS = new Set(["<", ">", '"']);
/** What is dropped from the end of an address: it belongs to the sentence, not the address. */
const LINK_TRAILING = new Set([".", ",", ";", ":", "!", "?", ")", "]", "}", "'", '"']);
/** Where the host ends and the rest of the address starts. */
const HOST_ENDS = new Set(["/", "?", "#"]);
/** The part before the first "/": a host of letters, digits, dots, and hyphens, then a port. */
const HOST = /^[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.?(?::[0-9]{1,5})?$/;

/** How long the start of a web address is ("http://" or "https://") at `at`, or 0 for none. */
function schemeLength(chars: readonly string[], at: number): number {
  const first = chars[at];
  if (first !== "h" && first !== "H") return 0; // the usual case
  let word = "";
  for (let k = at; k < at + 8 && k < chars.length; k++) {
    const char = chars[k] ?? "";
    // Only the ASCII capitals are made small: no other letter may pass for an h, t, p, or s.
    word += char >= "A" && char <= "Z" ? char.toLowerCase() : char;
  }
  if (word.startsWith("https://")) return 8;
  if (word.startsWith("http://")) return 7;
  return 0;
}

/**
 * Find the web addresses in text that was already marked. An address starts with `https://` or
 * `http://`, at the start, after a space, or after one of `([{<"'`. It runs to the next space,
 * hidden character, or one of `<>"`, loses the sentence marks at its end, and then must pass
 * the checks: host, no user name, not too long.
 *
 * The work is about the length of the text: each stretch is looked at once to find where it
 * ends, and an address that is too long is turned away without dropping its tail first.
 */
function findLinks({ chars, hidden }: Marked): Span[] {
  const count = chars.length;
  const stops = (at: number) => {
    const char = chars[at] ?? "";
    return hidden[at] !== null || SPACE.test(char) || LINK_STOPS.has(char);
  };

  // For each place: where its stretch (up to the next stop) ends.
  const stretchEnd: number[] = new Array<number>(count + 1).fill(count);
  // How many "(" and ")" came before each place, to tell if a ")" has its "(".
  const opens: number[] = [0];
  const closes: number[] = [0];
  for (let at = count - 1; at >= 0; at--) {
    stretchEnd[at] = stops(at) ? at : (stretchEnd[at + 1] ?? count);
  }
  for (const char of chars) {
    opens.push((opens[opens.length - 1] ?? 0) + (char === "(" ? 1 : 0));
    closes.push((closes[closes.length - 1] ?? 0) + (char === ")" ? 1 : 0));
  }
  const between = (sums: readonly number[], from: number, to: number) =>
    (sums[to] ?? 0) - (sums[from] ?? 0);

  // Where the sentence marks at the end of a stretch begin (kept for the stretch just read).
  let tailOf = -1;
  let tailStart = 0;
  const tailBegins = (end: number) => {
    if (tailOf !== end) {
      tailOf = end;
      tailStart = end;
      while (
        tailStart > 0 &&
        !stops(tailStart - 1) &&
        LINK_TRAILING.has(chars[tailStart - 1] ?? "")
      ) {
        tailStart -= 1;
      }
    }
    return tailStart;
  };

  /** Where the address that starts at `start` ends, or 0 when it is not an address to offer. */
  const addressEnd = (start: number, scheme: number) => {
    const stretch = stretchEnd[start] ?? count;
    const tail = tailBegins(stretch);
    // Too long even without its tail: no need to cut the tail off to know.
    if (tail - start > MAX_LINK_CHARS) return 0;
    let end = stretch;
    while (end > tail) {
      // A ")" stays when the address has a "(" for it: .../Rust_(language)
      if (chars[end - 1] === ")" && between(opens, start, end) >= between(closes, start, end))
        break;
      end -= 1;
    }
    if (end - start > MAX_LINK_CHARS) return 0;
    let hostEnd = start + scheme;
    while (hostEnd < end && !HOST_ENDS.has(chars[hostEnd] ?? "")) hostEnd += 1;
    // This also turns away a user name (user@host), a host with any other letter than ASCII, and
    // a backslash: a web browser reads it as a "/", so the host would be a different one.
    return HOST.test(chars.slice(start + scheme, hostEnd).join("")) ? end : 0;
  };

  const links: Span[] = [];
  let at = 0;
  while (at < count) {
    const scheme = schemeLength(chars, at);
    const before = chars[at - 1] ?? "";
    const startsHere =
      scheme > 0 &&
      (at === 0 || (hidden[at - 1] === null && (SPACE.test(before) || BEFORE_LINK.has(before))));
    const end = startsHere ? addressEnd(at, scheme) : 0;
    if (end > 0) {
      links.push({ start: at, end });
      at = end;
    } else {
      at += 1;
    }
  }
  return links;
}

/**
 * Cut `text` into parts to show (see the top of this file). Every character ends up in exactly
 * one part, in its place, so the parts put back together are the text again. An empty text has
 * no parts. Two text parts never sit side by side, and no text part is empty.
 */
export function pieces(text: string): Piece[] {
  const marked = mark(text);
  const { chars, hidden } = marked;
  const links = findLinks(marked);
  const parts: Piece[] = [];
  let plain = "";
  const flush = () => {
    if (plain !== "") parts.push({ kind: "text", text: plain });
    plain = "";
  };

  let nextLink = 0;
  let at = 0;
  while (at < chars.length) {
    const link = links[nextLink];
    if (link && link.start === at) {
      flush();
      const address = chars.slice(link.start, link.end).join("");
      parts.push({ kind: "link", text: address, url: address });
      at = link.end;
      nextLink += 1;
      continue;
    }
    const char = chars[at] ?? "";
    const name = hidden[at];
    if (typeof name === "string") {
      flush();
      parts.push({ kind: "hidden", code: codeOf(char), name, char });
    } else {
      plain += char;
    }
    at += 1;
  }
  flush();
  return parts;
}

/** How many hidden characters `text` has: the number of hidden parts `pieces` makes. */
export function hiddenCount(text: string): number {
  return mark(text).hidden.filter((name) => name !== null).length;
}

/**
 * The text on one line, for a small place such as a name in a list: every hidden character and
 * every line break becomes U+FFFD (the replacement mark), so it shows that something was there,
 * and one person's words can never take more than one line. A tab stays as it is.
 */
export function oneLine(text: string): string {
  const { chars, hidden } = mark(text);
  return chars
    .map((char, at) => (hidden[at] !== null || char === "\n" ? REPLACEMENT : char))
    .join("");
}
