import { SYSTEM_WORDS } from "@plenipo/types";
import { afterEach, describe, expect, it } from "vitest";

import { sentenceStart, setSystemWords, shortcut, systemWords } from "./words";

afterEach(() => setSystemWords(SYSTEM_WORDS.windows));

describe("each system's own words (ADR-155)", () => {
  it("uses Windows' words until the app says which system it is on", () => {
    expect(systemWords().thisComputer).toBe("this PC");
    expect(systemWords().system).toBe("windows");
  });

  it("uses the words the Rust side sent", () => {
    setSystemWords(SYSTEM_WORDS.mac);
    expect(systemWords().thisComputer).toBe("this Mac");
    expect(systemWords().showFile).toBe("Show in Finder");
    setSystemWords(SYSTEM_WORDS.linux);
    expect(systemWords().fileProgram).toBe("your file manager");
  });

  it("starts a sentence with the system", () => {
    expect(sentenceStart(SYSTEM_WORDS.windows.theSystem)).toBe("Windows");
    expect(sentenceStart(SYSTEM_WORDS.mac.theSystem)).toBe("Your Mac");
  });

  it("labels shortcuts with each system's keys", () => {
    expect(shortcut(["ctrl", "shift"], "E")).toBe("Ctrl+Shift+E");
    expect(shortcut(["mod"], "S")).toBe("Ctrl+S");
    setSystemWords(SYSTEM_WORDS.linux);
    expect(shortcut(["mod"], "S")).toBe("Ctrl+S");
    setSystemWords(SYSTEM_WORDS.mac);
    expect(shortcut(["ctrl", "shift"], "E")).toBe("⌃⇧E");
    expect(shortcut(["mod"], "S")).toBe("⌘S");
    expect(shortcut(["ctrl"], "`")).toBe("⌃`");
  });

  it("never says Windows or PC on a Mac or Linux", () => {
    for (const words of [SYSTEM_WORDS.mac, SYSTEM_WORDS.linux]) {
      const all = Object.values(words).join(" ");
      expect(all).not.toContain("Windows");
      expect(all).not.toMatch(/\bPC\b/);
    }
  });
});
