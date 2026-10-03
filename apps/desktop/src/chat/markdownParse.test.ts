import { describe, expect, it } from "vitest";

import {
  parseInline,
  parseMarkdown,
  parseParts,
  plainText,
  safeLink,
  type Block,
  type Inline,
} from "./markdownParse";

const words = (parts: Inline[]) => plainText(parts);

function only(source: string): Block {
  const blocks = parseMarkdown(source);
  expect(blocks).toHaveLength(1);
  return blocks[0] as Block;
}

describe("blocks", () => {
  it("reads paragraphs, keeping a line break where the agent made one", () => {
    const blocks = parseMarkdown("First line\nsecond line\n\nNext paragraph");
    expect(blocks.map((b) => b.t)).toEqual(["p", "p"]);
    const first = blocks[0];
    expect(first?.t === "p" && first.c.map((c) => c.t)).toEqual(["text", "br", "text"]);
  });

  it("reads headings, and leaves a hash with no space as words", () => {
    expect(only("## The plan")).toMatchObject({ t: "h", level: 2 });
    expect(only("#hashtag")).toMatchObject({ t: "p" });
  });

  it("reads a closed code block with its language", () => {
    expect(only("```ts\nconst a = 1;\n```")).toEqual({
      t: "code",
      lang: "ts",
      v: "const a = 1;",
      open: false,
    });
  });

  it("keeps a code block that is still being written open, and does not mark it up", () => {
    expect(only("```powershell\nRemove-Item **x** -Recurse")).toEqual({
      t: "code",
      lang: "powershell",
      v: "Remove-Item **x** -Recurse",
      open: true,
    });
  });

  it("closes a tilde fence only with a tilde fence", () => {
    const block = only("~~~\n```\n~~~");
    expect(block).toMatchObject({ t: "code", v: "```", open: false });
  });

  it("reads a rule, and a quote", () => {
    expect(only("---")).toEqual({ t: "hr" });
    const quote = only("> one\n> two");
    expect(quote.t === "quote" && quote.c).toHaveLength(1);
  });

  it("reads bullet lists and numbered lists with their first number", () => {
    const ul = only("- one\n- two\n- three");
    expect(ul.t === "ul" && ul.items).toHaveLength(3);
    const ol = only("3. three\n4. four");
    expect(ol).toMatchObject({ t: "ol", start: 3 });
  });

  it("starts a list right after a sentence with no blank line between", () => {
    const blocks = parseMarkdown("Here are the steps:\n1. Open it\n2. Save it");
    expect(blocks.map((b) => b.t)).toEqual(["p", "ol"]);
  });

  it("does not turn a year followed by a full stop into a list", () => {
    const blocks = parseMarkdown("It was fixed in\n2024. A good year");
    expect(blocks.map((b) => b.t)).toEqual(["p"]);
  });

  it("nests a list under an item, even when it is indented by two spaces under a number", () => {
    const list = only("1. Build\n  - compile\n  - link\n2. Ship");
    expect(list.t).toBe("ol");
    if (list.t !== "ol") return;
    expect(list.items).toHaveLength(2);
    const inner = list.items[0]?.blocks.find((b) => b.t === "ul");
    expect(inner?.t === "ul" && inner.items).toHaveLength(2);
  });

  it("keeps one list across blank lines between its items", () => {
    const list = only("- a\n\n- b\n\n- c");
    expect(list.t === "ul" && list.items).toHaveLength(3);
  });

  it("reads task lines", () => {
    const list = only("- [x] done\n- [ ] to do");
    expect(list.t === "ul" && list.items.map((i) => i.task)).toEqual([true, false]);
  });

  it("runs a line on into the item above it", () => {
    const list = only("- first part\nsecond part");
    expect(list.t === "ul" && list.items).toHaveLength(1);
  });

  it("reads a table with its alignment, and pads short rows", () => {
    const table = only("| Name | Size |\n| :-- | --: |\n| a | 1 |\n| b |");
    expect(table.t).toBe("table");
    if (table.t !== "table") return;
    expect(table.align).toEqual(["left", "right"]);
    expect(table.head.map(words)).toEqual(["Name", "Size"]);
    expect(table.rows).toHaveLength(2);
    expect(table.rows[1]?.map(words)).toEqual(["b", ""]);
  });

  it("does not turn a line with a pipe into a table unless a rule line follows", () => {
    expect(only("a | b")).toMatchObject({ t: "p" });
  });

  it("stops reading nested quotes and lists at a safe depth", () => {
    const deep = `${"> ".repeat(40)}end`;
    expect(() => parseMarkdown(deep)).not.toThrow();
    const lists = `${Array.from({ length: 40 }, (_, i) => `${"  ".repeat(i)}- x`).join("\n")}`;
    expect(() => parseMarkdown(lists)).not.toThrow();
  });
});

describe("inline", () => {
  it("reads bold, italic, struck-out, and code", () => {
    expect(parseInline("**b** *i* ~~s~~ `c`").map((p) => p.t)).toEqual([
      "strong",
      "text",
      "em",
      "text",
      "del",
      "text",
      "code",
    ]);
  });

  it("reads bold with italic inside, and three marks as both", () => {
    const nested = parseInline("**bold *and italic* too**");
    expect(nested).toHaveLength(1);
    expect(nested[0]).toMatchObject({ t: "strong" });
    expect(parseInline("***both***")[0]).toMatchObject({ t: "em" });
  });

  it("shows marks that never close, as they are (text that is still being written)", () => {
    expect(parseInline("a **bold that is not done")).toEqual([
      { t: "text", v: "a **bold that is not done" },
    ]);
  });

  it("does not italicize inside a word or around spaces", () => {
    expect(words(parseInline("clear_temp_files and 2 * 3 * 4"))).toBe(
      "clear_temp_files and 2 * 3 * 4",
    );
    expect(parseInline("clear_temp_files").some((p) => p.t === "em")).toBe(false);
  });

  it("does not look for marks inside code", () => {
    const parts = parseInline("`a **b` c**");
    expect(parts[0]).toEqual({ t: "code", v: "a **b" });
    expect(parts.some((p) => p.t === "strong")).toBe(false);
  });

  it("reads double backticks around a backtick", () => {
    expect(parseInline("``a ` b``")).toEqual([{ t: "code", v: "a ` b" }]);
  });

  it("takes a backslash off a mark", () => {
    expect(words(parseInline("\\*not italic\\*"))).toBe("*not italic*");
  });

  it("keeps a web link and shows other links as their words only", () => {
    expect(parseInline("[docs](https://example.com/a)")).toEqual([
      { t: "link", href: "https://example.com/a", c: [{ t: "text", v: "docs" }] },
    ]);
    expect(parseInline("[click](javascript:alert(1))").some((p) => p.t === "link")).toBe(false);
    expect(words(parseInline("[click](javascript:alert(1))"))).toBe("click");
    expect(parseInline("[file](data:text/html;base64,AAAA)").some((p) => p.t === "link")).toBe(
      false,
    );
    expect(parseInline("[x](file:///C:/Windows)").some((p) => p.t === "link")).toBe(false);
    expect(parseInline("[x](../secret.txt)").some((p) => p.t === "link")).toBe(false);
  });

  it("never loads a picture an agent names", () => {
    const parts = parseInline("![logo](https://example.com/logo.png)");
    expect(parts.some((p) => p.t === "link")).toBe(false);
    expect(words(parts)).toBe("[picture: logo]");
  });

  it("links a bare web address but not the full stop after it", () => {
    const parts = parseInline("See https://example.com/path. Then go on");
    const link = parts.find((p) => p.t === "link");
    expect(link).toMatchObject({ href: "https://example.com/path" });
    expect(words(parts)).toBe("See https://example.com/path. Then go on");
  });

  it("keeps a closing bracket that belongs to the address", () => {
    const parts = parseInline("(see https://en.wikipedia.org/wiki/Rust_(language))");
    const link = parts.find((p) => p.t === "link");
    expect(link).toMatchObject({ href: "https://en.wikipedia.org/wiki/Rust_(language)" });
  });

  it("handles a lot of odd marks without hanging", () => {
    const text = "*a ".repeat(20_000);
    const start = Date.now();
    parseInline(text);
    expect(Date.now() - start).toBeLessThan(3000);
  });
});

describe("safeLink", () => {
  it("allows web addresses and mail, nothing else", () => {
    expect(safeLink("https://example.com")).toBe("https://example.com/");
    expect(safeLink("http://example.com/x")).toBe("http://example.com/x");
    expect(safeLink("mailto:me@example.com")).toBe("mailto:me@example.com");
    for (const bad of [
      "javascript:alert(1)",
      "JaVaScRiPt:alert(1)",
      "vbscript:x",
      "data:text/html,<b>",
      "file:///etc/passwd",
      "ftp://example.com",
      "//example.com",
      "example.com",
      "https://exa mple.com",
      "https://example.com/\u0000",
      "",
    ]) {
      expect(safeLink(bad)).toBeNull();
    }
  });
});

describe("parts (reusing what did not change while text streams in)", () => {
  it("gives each top-level block the exact text it came from", () => {
    const parts = parseParts("# Title\n\nSome words\n\n```\ncode\n```");
    expect(parts.map((p) => p.raw)).toEqual(["# Title", "Some words", "```\ncode\n```"]);
  });

  it("changes only the last block while the text grows", () => {
    const before = parseParts("# Title\n\nFirst paragraph\n\nSecond par");
    const after = parseParts("# Title\n\nFirst paragraph\n\nSecond paragraph, longer");
    expect(after.slice(0, 2).map((p) => p.raw)).toEqual(before.slice(0, 2).map((p) => p.raw));
    expect(after[2]?.raw).not.toBe(before[2]?.raw);
  });
});
