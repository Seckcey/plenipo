import { describe, expect, it } from "vitest";

import {
  baseName,
  describeCalls,
  elapsed,
  fileOf,
  thoughtFor,
  toolKind,
  toolName,
  toolPhrase,
} from "./words";

describe("tool names", () => {
  it("strips the prefix Claude Code puts on Plenipo's tools", () => {
    expect(toolName("mcp__plenipo__write_file")).toBe("write_file");
    expect(toolName("write_file")).toBe("write_file");
    expect(toolName("mcp__other__a__b")).toBe("a__b");
  });

  it("sorts tools into what they do", () => {
    expect(toolKind("mcp__plenipo__read_file")).toBe("read");
    expect(toolKind("run_powershell")).toBe("script");
    expect(toolKind("git_commit")).toBe("git");
    expect(toolKind("browser_open")).toBe("web");
    expect(toolKind("screen_click")).toBe("screen");
    expect(toolKind("ssh_run")).toBe("server");
    expect(toolKind("something_new")).toBe("other");
  });

  it("says what a tool is doing, now and after", () => {
    expect(toolPhrase("write_file", true)).toBe("Saving a file");
    expect(toolPhrase("write_file", false)).toBe("Saved a file");
    expect(toolPhrase("run_command", true)).toBe("Running a program");
    expect(toolPhrase("odd_tool", true)).toBe("Using odd_tool");
  });
});

describe("one sentence for a run of calls", () => {
  it("names one file and counts several", () => {
    expect(
      describeCalls(
        [
          { tool: "read_file", summary: "src/Inspector.tsx" },
          { tool: "run_command", summary: "cargo test" },
          { tool: "run_command", summary: "cargo fmt" },
          { tool: "search_text", summary: "grant_skipped" },
        ],
        true,
      ),
    ).toBe("Read Inspector.tsx, ran 2 programs, searched the files");
    expect(
      describeCalls(
        [
          { tool: "read_file", summary: "a.txt" },
          { tool: "read_file", summary: "b.txt" },
        ],
        true,
      ),
    ).toBe("Read 2 files");
    expect(describeCalls([{ tool: "write_file", summary: "clear-temp.ps1" }], true)).toBe(
      "Saved clear-temp.ps1",
    );
  });

  it("uses the words for what is happening now", () => {
    expect(describeCalls([{ tool: "run_powershell", summary: "clear-temp.ps1" }], false)).toBe(
      "Running a script",
    );
  });

  it("lists a tool of its own by name", () => {
    expect(describeCalls([{ tool: "microsoft_mail_search", summary: "invoice" }], true)).toBe(
      "Used microsoft_mail_search",
    );
  });
});

describe("files and times", () => {
  it("finds a path in a summary, and leaves a sentence alone", () => {
    expect(fileOf("src/pages/Home.tsx")).toBe("src/pages/Home.tsx");
    expect(fileOf("notes.md")).toBe("notes.md");
    expect(fileOf("cargo test --workspace")).toBeNull();
    expect(fileOf("")).toBeNull();
    expect(baseName("C:\\Users\\me\\a.ps1")).toBe("a.ps1");
    expect(baseName("a/b/c.txt")).toBe("c.txt");
  });

  it("says seconds, minutes, and hours the way a person does", () => {
    expect(elapsed(0)).toBe("0 s");
    expect(elapsed(8_400)).toBe("8 s");
    expect(elapsed(138_000)).toBe("2m 18s");
    expect(elapsed(3_900_000)).toBe("1h 05m");
    expect(elapsed(-5)).toBe("0 s");
    expect(thoughtFor(6_000)).toBe("Thought for 6 s");
  });
});
