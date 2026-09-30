import { cleanup, render } from "@testing-library/react";
import type { EditorState } from "@codemirror/state";
import { afterEach, describe, expect, it, vi } from "vitest";

import { CodeEditor } from "./CodeEditor";
import { editorState } from "./editorSetup";

afterEach(cleanup);

const texts = (calls: unknown[][]) => calls.map(([s]) => (s as EditorState).doc.toString());

describe("the editor itself (CodeMirror, Phase 21, ADR-093 §5)", () => {
  it("gives back what was typed when it closes, never text the page replaced", () => {
    const onChange = vi.fn();
    const props = {
      fileName: "notes.txt",
      readOnly: false,
      shown: null,
      marks: [],
      label: "notes.txt, editable",
      onChange,
    };
    const first = editorState("what you typed", false);
    const { rerender, unmount } = render(<CodeEditor {...props} state={first} />);
    // Reload, or a worker's change read again: the page gives the file as it is now.
    const now = editorState("the file now", false);
    rerender(<CodeEditor {...props} state={now} />);
    expect(texts(onChange.mock.calls)).not.toContain("what you typed");
    // The page goes: the editor gives back what it holds now.
    unmount();
    expect(texts(onChange.mock.calls)).toEqual(["the file now"]);
  });

  it("keeps a lone carriage return inside a line as it is", () => {
    const state = editorState("one\rstill one\ntwo\n", false);
    expect(state.doc.lines).toBe(3);
    expect(state.doc.toString()).toBe("one\rstill one\ntwo\n");
  });
});
