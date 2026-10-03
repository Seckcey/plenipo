/**
 * A small, safe reader for the formatting an agent writes in a conversation: paragraphs, headings,
 * lists, code, quotes, tables, bold, italic, and links. It builds plain data, never HTML, so
 * nothing an agent writes can run (the screens draw it with React, which escapes everything).
 *
 * It copes with text that is still being written: an open code block stays a code block, and a
 * `**` that has not closed yet is shown as it is. Only these links are kept: web addresses
 * (`http`, `https`) and mail (`mailto`). Anything else is shown as words.
 */

export type Inline =
  | { t: "text"; v: string }
  | { t: "code"; v: string }
  | { t: "strong"; c: Inline[] }
  | { t: "em"; c: Inline[] }
  | { t: "del"; c: Inline[] }
  | { t: "link"; href: string; c: Inline[] }
  | { t: "br" };

export type Align = "left" | "center" | "right" | null;

export interface ListItem {
  /** `true` or `false` for a task line (`[x]`, `[ ]`); `null` for an ordinary item. */
  task: boolean | null;
  blocks: Block[];
}

export type Block =
  | { t: "p"; c: Inline[] }
  | { t: "h"; level: 1 | 2 | 3 | 4 | 5 | 6; c: Inline[] }
  /** `open`: the closing fence has not arrived yet (the code is still being written). */
  | { t: "code"; lang: string; v: string; open: boolean }
  | { t: "ul"; items: ListItem[] }
  | { t: "ol"; start: number; items: ListItem[] }
  | { t: "quote"; c: Block[] }
  | { t: "hr" }
  | { t: "table"; align: Align[]; head: Inline[][]; rows: Inline[][][] };

/** A top-level block with the exact text it came from, so a drawn block can be reused. */
export interface Part {
  raw: string;
  block: Block;
}

/** Deepest nesting of lists and quotes that is read; deeper text is shown as plain words. */
const MAX_DEPTH = 6;
/** Longest address that becomes a link. */
const MAX_LINK = 2048;

const FENCE = /^( {0,3})(`{3,}|~{3,})([^`]*)$/;
const CLOSE_FENCE = /^ {0,3}(`{3,}|~{3,})[ \t]*$/;
const HEADING = /^ {0,3}(#{1,6})(?:[ \t]+(.*?))?(?:[ \t]+#+)?[ \t]*$/;
const RULE = /^ {0,3}([-*_])(?:[ \t]*\1){2,}[ \t]*$/;
const QUOTE = /^ {0,3}>[ ]?(.*)$/;
const LIST_MARKER = /^( {0,3})([-*+]|\d{1,9}[.)])(?:([ \t]+)(.*))?$/;
const TABLE_RULE = /^[ \t]*\|?[ \t]*:?-+:?[ \t]*(?:\|[ \t]*:?-+:?[ \t]*)*\|?[ \t]*$/;
const TASK = /^\[([ xX])\][ \t]+(.*)$/;

/** Reads text into blocks. */
export function parseMarkdown(source: string): Block[] {
  return parseParts(source).map((p) => p.block);
}

/** Reads text into top-level blocks, each with the text it came from. */
export function parseParts(source: string): Part[] {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const parts: Part[] = [];
  readBlocks(lines, 0, (block, from, to) => {
    parts.push({ raw: lines.slice(from, to).join("\n"), block });
  });
  return parts;
}

function line(lines: string[], i: number): string {
  return lines[i] ?? "";
}

function isBlank(text: string): boolean {
  return text.trim() === "";
}

function indentOf(text: string): number {
  let n = 0;
  for (const ch of text) {
    if (ch === " ") n += 1;
    else if (ch === "\t") n += 4;
    else break;
  }
  return n;
}

/** Removes up to `n` columns of leading space. */
function dedent(text: string, n: number): string {
  let cut = 0;
  let cols = 0;
  while (cut < text.length && cols < n) {
    const ch = text[cut];
    if (ch === " ") cols += 1;
    else if (ch === "\t") cols += 4;
    else break;
    cut += 1;
  }
  return text.slice(cut);
}

function readBlocks(
  lines: string[],
  depth: number,
  emit: (block: Block, from: number, to: number) => void,
): void {
  let i = 0;
  while (i < lines.length) {
    const text = line(lines, i);
    if (isBlank(text)) {
      i += 1;
      continue;
    }
    const from = i;

    const fence = FENCE.exec(text);
    if (fence) {
      const marker = fence[2] ?? "```";
      const lang = (fence[3] ?? "").trim().split(/\s+/)[0] ?? "";
      const indent = (fence[1] ?? "").length;
      const body: string[] = [];
      let j = i + 1;
      let closed = false;
      while (j < lines.length) {
        const l = line(lines, j);
        const close = CLOSE_FENCE.exec(l);
        const run = close?.[1] ?? "";
        if (run.startsWith(marker.charAt(0)) && run.length >= marker.length) {
          closed = true;
          j += 1;
          break;
        }
        body.push(dedent(l, indent));
        j += 1;
      }
      emit({ t: "code", lang, v: body.join("\n"), open: !closed }, from, j);
      i = j;
      continue;
    }

    const heading = HEADING.exec(text);
    if (heading) {
      const level = (heading[1] ?? "#").length as 1 | 2 | 3 | 4 | 5 | 6;
      emit({ t: "h", level, c: parseInline(heading[2] ?? "") }, from, i + 1);
      i += 1;
      continue;
    }

    if (RULE.test(text)) {
      emit({ t: "hr" }, from, i + 1);
      i += 1;
      continue;
    }

    if (QUOTE.test(text) && depth < MAX_DEPTH) {
      const inner: string[] = [];
      let j = i;
      while (j < lines.length) {
        const m = QUOTE.exec(line(lines, j));
        if (!m) break;
        inner.push(m[1] ?? "");
        j += 1;
      }
      const c: Block[] = [];
      readBlocks(inner, depth + 1, (b) => c.push(b));
      emit({ t: "quote", c }, from, j);
      i = j;
      continue;
    }

    if (isTableStart(lines, i)) {
      const end = readTable(lines, i, emit);
      i = end;
      continue;
    }

    if (LIST_MARKER.test(text) && depth < MAX_DEPTH) {
      i = readList(lines, i, depth, emit);
      continue;
    }

    // A paragraph runs until a blank line or the start of another kind of block.
    let j = i + 1;
    while (j < lines.length) {
      const l = line(lines, j);
      if (isBlank(l) || startsBlock(lines, j)) break;
      j += 1;
    }
    const words = lines
      .slice(i, j)
      .map((l) => l.trim())
      .join("\n");
    emit({ t: "p", c: parseInline(words) }, from, j);
    i = j;
  }
}

/** Does a new block start on this line (so it ends a paragraph above it)? */
function startsBlock(lines: string[], i: number): boolean {
  const text = line(lines, i);
  return (
    FENCE.test(text) ||
    HEADING.test(text) ||
    RULE.test(text) ||
    QUOTE.test(text) ||
    isListStart(text) ||
    isTableStart(lines, i)
  );
}

/** A list item can end a paragraph above it; an ordinary number with a full stop must not. */
function isListStart(text: string): boolean {
  const m = LIST_MARKER.exec(text);
  if (!m) return false;
  const marker = m[2] ?? "";
  if (/^\d/.test(marker)) return m[4] !== undefined && marker.length <= 3;
  return m[4] !== undefined && m[4] !== "";
}

function splitRow(text: string): string[] {
  let row = text.trim();
  if (row.startsWith("|")) row = row.slice(1);
  if (row.endsWith("|") && !row.endsWith("\\|")) row = row.slice(0, -1);
  const cells: string[] = [];
  let cell = "";
  for (let k = 0; k < row.length; k += 1) {
    const ch = row.charAt(k);
    if (ch === "\\" && row.charAt(k + 1) === "|") {
      cell += "|";
      k += 1;
    } else if (ch === "|") {
      cells.push(cell.trim());
      cell = "";
    } else {
      cell += ch;
    }
  }
  cells.push(cell.trim());
  return cells;
}

function isTableStart(lines: string[], i: number): boolean {
  const head = line(lines, i);
  const rule = line(lines, i + 1);
  if (!head.includes("|") || !rule.includes("-") || !TABLE_RULE.test(rule)) return false;
  return splitRow(head).length === splitRow(rule).length;
}

function readTable(
  lines: string[],
  i: number,
  emit: (block: Block, from: number, to: number) => void,
): number {
  const align: Align[] = splitRow(line(lines, i + 1)).map((cell) => {
    const left = cell.startsWith(":");
    const right = cell.endsWith(":");
    if (left && right) return "center";
    if (right) return "right";
    return left ? "left" : null;
  });
  const columns = align.length;
  const head = splitRow(line(lines, i)).map((c) => parseInline(c));
  const rows: Inline[][][] = [];
  let j = i + 2;
  while (j < lines.length) {
    const l = line(lines, j);
    if (isBlank(l) || !l.includes("|")) break;
    const cells = splitRow(l).map((c) => parseInline(c));
    while (cells.length < columns) cells.push([]);
    rows.push(cells.slice(0, columns));
    j += 1;
  }
  emit({ t: "table", align, head, rows }, i, j);
  return j;
}

function readList(
  lines: string[],
  start: number,
  depth: number,
  emit: (block: Block, from: number, to: number) => void,
): number {
  const first = LIST_MARKER.exec(line(lines, start));
  const firstMarker = first?.[2] ?? "-";
  const ordered = /^\d/.test(firstMarker);
  const items: ListItem[] = [];
  let i = start;
  while (i < lines.length) {
    const m = LIST_MARKER.exec(line(lines, i));
    if (!m || /^\d/.test(m[2] ?? "") !== ordered) break;
    const markerIndent = (m[1] ?? "").length;
    const gap = m[3] ? Math.min(indentOf(m[3]), 4) : 1;
    const contentIndent = markerIndent + (m[2] ?? "").length + (m[4] ? gap : 1);
    const itemLines: string[] = [m[4] ?? ""];
    i += 1;
    let afterBlank = false;
    while (i < lines.length) {
      const l = line(lines, i);
      if (isBlank(l)) {
        itemLines.push("");
        afterBlank = true;
        i += 1;
        continue;
      }
      const indent = indentOf(l);
      if (indent >= contentIndent) {
        itemLines.push(dedent(l, contentIndent));
      } else if (indent > markerIndent && LIST_MARKER.test(dedent(l, indent))) {
        // A sub-list indented less than the item's words (two spaces under "1.") still nests.
        itemLines.push(dedent(l, indent));
      } else if (!afterBlank && !startsBlock(lines, i)) {
        // A line that runs on from the item's last paragraph.
        itemLines.push(l.trim());
      } else {
        break;
      }
      afterBlank = false;
      i += 1;
    }
    while (itemLines.length > 0 && isBlank(itemLines[itemLines.length - 1] ?? "")) itemLines.pop();
    items.push(buildItem(itemLines, depth));
    // Blank lines between two items of one list do not end the list.
    let k = i;
    while (k < lines.length && isBlank(line(lines, k))) k += 1;
    const next = LIST_MARKER.exec(line(lines, k));
    if (!next || /^\d/.test(next[2] ?? "") !== ordered || (next[1] ?? "").length > 3) break;
    i = k;
  }
  if (ordered) {
    emit({ t: "ol", start: Number.parseInt(firstMarker, 10) || 1, items }, start, i);
  } else {
    emit({ t: "ul", items }, start, i);
  }
  return i;
}

function buildItem(itemLines: string[], depth: number): ListItem {
  let task: boolean | null = null;
  const head = itemLines[0] ?? "";
  const m = TASK.exec(head);
  if (m) {
    task = (m[1] ?? " ") !== " ";
    itemLines[0] = m[2] ?? "";
  }
  const blocks: Block[] = [];
  readBlocks(itemLines, depth + 1, (b) => blocks.push(b));
  return { task, blocks };
}

// ---- Inline ---------------------------------------------------------------------------------

const ESCAPABLE = "\\`*_{}[]()#+-.!|~<>\"'$%&,/:;=?@^";

/** Takes the full stop, comma, or closing bracket that ends a sentence off a web address. */
function trimAddress(address: string): string {
  let text = address;
  for (;;) {
    const last = text.charAt(text.length - 1);
    const pair = last === ")" ? "(" : last === "]" ? "[" : last === "}" ? "{" : "";
    const stray = pair !== "" && text.split(last).length > text.split(pair).length;
    if (".,;:!?'\"".includes(last) || stray) text = text.slice(0, -1);
    else return text;
  }
}

function isSpace(ch: string): boolean {
  return ch === "" || /\s/.test(ch);
}

function isWordChar(ch: string): boolean {
  return /[\p{L}\p{N}]/u.test(ch);
}

/** Does the text hold a space, a line break, or a hidden control mark? */
function hasBlankOrControl(text: string): boolean {
  for (const ch of text) {
    const code = ch.codePointAt(0) ?? 0;
    if (code <= 0x20 || code === 0x7f || /\s/.test(ch)) return true;
  }
  return false;
}

/** The only addresses that may become links. */
export function safeLink(address: string): string | null {
  const text = address.trim();
  if (text === "" || text.length > MAX_LINK || hasBlankOrControl(text)) return null;
  try {
    const url = new URL(text);
    if (url.protocol === "http:" || url.protocol === "https:") return url.href;
    if (url.protocol === "mailto:") return url.href;
  } catch {
    return null;
  }
  return null;
}

/** Reads one line or paragraph of text into pieces. */
export function parseInline(source: string, depth = 0): Inline[] {
  const out: Inline[] = [];
  let buf = "";
  const flush = () => {
    if (buf !== "") {
      out.push({ t: "text", v: buf });
      buf = "";
    }
  };
  const s = source;
  let i = 0;
  while (i < s.length) {
    const ch = s.charAt(i);

    if (ch === "\\") {
      const next = s.charAt(i + 1);
      if (next === "\n") {
        flush();
        out.push({ t: "br" });
        i += 2;
        continue;
      }
      if (next !== "" && ESCAPABLE.includes(next)) {
        buf += next;
        i += 2;
        continue;
      }
      buf += ch;
      i += 1;
      continue;
    }

    if (ch === "\n") {
      flush();
      out.push({ t: "br" });
      i += 1;
      continue;
    }

    if (ch === "`") {
      let run = 1;
      while (s.charAt(i + run) === "`") run += 1;
      const fence = "`".repeat(run);
      let close = s.indexOf(fence, i + run);
      while (close !== -1 && s.charAt(close + run) === "`") {
        // A longer run of backticks is not this one's end.
        let longer = run;
        while (s.charAt(close + longer) === "`") longer += 1;
        close = s.indexOf(fence, close + longer);
      }
      if (close === -1) {
        buf += fence;
        i += run;
        continue;
      }
      let code = s.slice(i + run, close).replace(/\n/g, " ");
      if (code.length > 1 && code.startsWith(" ") && code.endsWith(" ") && code.trim() !== "") {
        code = code.slice(1, -1);
      }
      flush();
      out.push({ t: "code", v: code });
      i = close + run;
      continue;
    }

    if (ch === "!" && s.charAt(i + 1) === "[") {
      // Pictures from an agent are not loaded; they are shown as their description.
      const link = readLink(s, i + 1);
      if (link) {
        buf += link.label === "" ? "[picture]" : `[picture: ${link.label}]`;
        i = link.end;
        continue;
      }
    }

    if (ch === "[" && depth < MAX_DEPTH) {
      const link = readLink(s, i);
      if (link) {
        const href = safeLink(link.target);
        flush();
        const label = parseInline(link.label, depth + 1);
        if (href) out.push({ t: "link", href, c: label });
        else out.push(...label);
        i = link.end;
        continue;
      }
    }

    if (ch === "h" && (s.startsWith("http://", i) || s.startsWith("https://", i))) {
      const before = s.charAt(i - 1);
      if (i === 0 || !isWordChar(before)) {
        let end = i;
        while (end < s.length && !/[\s<>"`]/.test(s.charAt(end))) end += 1;
        const address = trimAddress(s.slice(i, end));
        const href = safeLink(address);
        if (href && address.length > 8) {
          flush();
          out.push({ t: "link", href, c: [{ t: "text", v: address }] });
          i += address.length;
          continue;
        }
      }
    }

    if ((ch === "*" || ch === "_" || ch === "~") && depth < MAX_DEPTH) {
      const made = readEmphasis(s, i, depth);
      if (made) {
        flush();
        out.push(made.node);
        i = made.end;
        continue;
      }
      let run = 1;
      while (s.charAt(i + run) === ch) run += 1;
      buf += ch.repeat(run);
      i += run;
      continue;
    }

    buf += ch;
    i += 1;
  }
  flush();
  return out;
}

/** `[label](address "title")` starting at `i` (which is the `[`). */
function readLink(s: string, i: number): { label: string; target: string; end: number } | null {
  let level = 0;
  let j = i;
  for (; j < s.length; j += 1) {
    const ch = s.charAt(j);
    if (ch === "\\") {
      j += 1;
      continue;
    }
    if (ch === "[") level += 1;
    else if (ch === "]") {
      level -= 1;
      if (level === 0) break;
    }
  }
  if (j >= s.length || s.charAt(j + 1) !== "(") return null;
  const label = s.slice(i + 1, j);
  let k = j + 2;
  let depth = 1;
  let target = "";
  for (; k < s.length; k += 1) {
    const ch = s.charAt(k);
    if (ch === "\n") return null;
    if (ch === "(") depth += 1;
    else if (ch === ")") {
      depth -= 1;
      if (depth === 0) break;
    }
    target += ch;
  }
  if (k >= s.length) return null;
  let address = target.trim();
  const titled = /^(\S+)\s+(?:"[^"]*"|'[^']*')$/.exec(address);
  if (titled) address = titled[1] ?? address;
  if (address.startsWith("<") && address.endsWith(">")) address = address.slice(1, -1);
  return { label, target: address, end: k + 1 };
}

/** Most characters one run of emphasis is looked for across; keeps odd text from being slow. */
const EMPHASIS_REACH = 4000;

/** Bold, italic, and struck-out text starting at `i`. */
function readEmphasis(s: string, i: number, depth: number): { node: Inline; end: number } | null {
  const ch = s.charAt(i);
  let run = 1;
  while (s.charAt(i + run) === ch) run += 1;
  if (run > 3 || (ch === "~" && run !== 2)) return null;
  // The opening marks must touch the words they hold, and `_` never opens inside a word.
  if (isSpace(s.charAt(i + run))) return null;
  if (ch === "_" && isWordChar(s.charAt(i - 1))) return null;
  const close = findCloser(s, i + run, ch, run);
  if (close === -1) return null;
  const content = parseInline(s.slice(i + run, close), depth + 1);
  const end = close + run;
  if (ch === "~") return { node: { t: "del", c: content }, end };
  if (run === 1) return { node: { t: "em", c: content }, end };
  if (run === 2) return { node: { t: "strong", c: content }, end };
  return { node: { t: "em", c: [{ t: "strong", c: content }] }, end };
}

/** Where the marks that close a run of emphasis start, or -1. Code and escapes are stepped over. */
function findCloser(s: string, from: number, ch: string, run: number): number {
  const limit = Math.min(s.length, from + EMPHASIS_REACH);
  let p = from;
  while (p < limit) {
    const c = s.charAt(p);
    if (c === "\\") {
      p += 2;
      continue;
    }
    if (c === "`") {
      let n = 1;
      while (s.charAt(p + n) === "`") n += 1;
      const end = s.indexOf("`".repeat(n), p + n);
      p = end === -1 ? p + n : end + n;
      continue;
    }
    if (c === ch) {
      let n = 1;
      while (s.charAt(p + n) === ch) n += 1;
      const closes =
        n === run &&
        p > from &&
        !isSpace(s.charAt(p - 1)) &&
        !(ch === "_" && isWordChar(s.charAt(p + n)));
      if (closes) return p;
      p += n;
      continue;
    }
    p += 1;
  }
  return -1;
}

/** The words of some inline pieces with no marks, for a label read aloud or a title. */
export function plainText(parts: Inline[]): string {
  let out = "";
  for (const p of parts) {
    switch (p.t) {
      case "text":
      case "code":
        out += p.v;
        break;
      case "br":
        out += " ";
        break;
      default:
        out += plainText(p.c);
    }
  }
  return out;
}
