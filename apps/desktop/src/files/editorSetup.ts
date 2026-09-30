/**
 * The editor's setup (CodeMirror 6, ADR-093 §5): its colors and look, the marks on a worker's new
 * and changed lines, read-only, Ctrl+S, and a new file's contents. The component is `CodeEditor`.
 */
import { basicSetup } from "codemirror";
import { indentWithTab } from "@codemirror/commands";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { Compartment, EditorState, RangeSetBuilder, type Extension } from "@codemirror/state";
import { Decoration, EditorView, GutterMarker, gutter, keymap } from "@codemirror/view";
import { tags } from "@lezer/highlight";

/** How a line is marked while a worker's change shows (ADR-055 §4): a bar and a word. */
export type LineMark = "new" | "changed" | null;

/** The page hears this when Ctrl+S is pressed in the editor. */
export const SAVE_EVENT = "plenipo:save";

export const readOnlyPart = new Compartment();
export const languagePart = new Compartment();
export const marksPart = new Compartment();

/** Colors for code, from the design tokens (they follow the light and dark themes). */
const highlight = HighlightStyle.define([
  { tag: [tags.keyword, tags.modifier, tags.operatorKeyword], color: "var(--ui-terminal-magenta)" },
  { tag: [tags.string, tags.special(tags.string), tags.regexp], color: "var(--ui-terminal-green)" },
  { tag: [tags.number, tags.bool, tags.null, tags.atom], color: "var(--ui-terminal-yellow)" },
  { tag: [tags.comment, tags.meta], color: "var(--ui-text-muted)", fontStyle: "italic" },
  {
    tag: [tags.function(tags.variableName), tags.function(tags.propertyName)],
    color: "var(--ui-terminal-blue)",
  },
  { tag: [tags.typeName, tags.className, tags.namespace], color: "var(--ui-terminal-cyan)" },
  { tag: [tags.tagName, tags.heading], color: "var(--ui-terminal-red)", fontWeight: "bold" },
  { tag: [tags.attributeName, tags.propertyName], color: "var(--ui-terminal-bright-blue)" },
  { tag: tags.link, color: "var(--ui-accent)", textDecoration: "underline" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strong, fontWeight: "bold" },
  { tag: tags.invalid, color: "var(--ui-terminal-bright-red)" },
]);

const look = EditorView.theme({
  "&": {
    height: "100%",
    color: "var(--ui-text-primary)",
    backgroundColor: "var(--ui-surface)",
    fontSize: "var(--ui-font-sm)",
  },
  ".cm-scroller": { fontFamily: "var(--ui-font-mono)", lineHeight: "1.5" },
  ".cm-content": { caretColor: "var(--ui-accent)" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--ui-accent)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "var(--ui-accent-soft)",
  },
  ".cm-gutters": {
    backgroundColor: "var(--ui-surface-sunken)",
    color: "var(--ui-text-muted)",
    borderRight: "1px solid var(--ui-border)",
  },
  ".cm-activeLine": { backgroundColor: "var(--ui-surface-raised)" },
  ".cm-activeLineGutter": { backgroundColor: "var(--ui-surface-raised)" },
  ".cm-panels": { backgroundColor: "var(--ui-surface-raised)", color: "var(--ui-text-primary)" },
  ".cm-line-new": { boxShadow: "inset 3px 0 0 var(--ui-ok)" },
  ".cm-line-changed": { boxShadow: "inset 3px 0 0 var(--ui-warn)" },
  ".cm-watch-mark": { padding: "0 4px", fontSize: "var(--ui-font-xs)" },
});

class MarkWord extends GutterMarker {
  constructor(private readonly mark: "new" | "changed") {
    super();
  }
  override eq(other: GutterMarker) {
    return other instanceof MarkWord && other.mark === this.mark;
  }
  override toDOM() {
    const el = document.createElement("span");
    el.textContent = this.mark === "new" ? "new" : "changed";
    el.title = this.mark === "new" ? "A new line" : "A changed line";
    return el;
  }
}

/** New and changed lines: a bar on the line and a word beside it (never color alone). */
export function marksExtension(marks: readonly LineMark[]): Extension {
  if (marks.length === 0 || marks.every((m) => m === null)) return [];
  const lineDecorations = EditorView.decorations.compute(["doc"], (state) => {
    const builder = new RangeSetBuilder<Decoration>();
    for (let n = 1; n <= state.doc.lines && n <= marks.length; n++) {
      const mark = marks[n - 1];
      if (!mark) continue;
      const line = state.doc.line(n);
      builder.add(line.from, line.from, Decoration.line({ class: `cm-line-${mark}` }));
    }
    return builder.finish();
  });
  const words = gutter({
    class: "cm-watch-mark",
    lineMarker(view, line) {
      const n = view.state.doc.lineAt(line.from).number;
      const mark = marks[n - 1];
      return mark ? new MarkWord(mark) : null;
    },
  });
  return [lineDecorations, words];
}

/**
 * Read-only: nothing can be typed or pasted, and a screen reader is told so. The text can still
 * be chosen, copied, and searched with the keyboard.
 */
export function readOnlyExtension(readOnly: boolean): Extension {
  return readOnly
    ? [EditorState.readOnly.of(true), EditorView.contentAttributes.of({ "aria-readonly": "true" })]
    : [];
}

/** A new editor's contents: its text, and everything the editor does. */
export function editorState(text: string, readOnly: boolean): EditorState {
  return EditorState.create({
    doc: text,
    extensions: [
      basicSetup,
      keymap.of([
        indentWithTab,
        {
          key: "Mod-s",
          preventDefault: true,
          run: (view) => {
            view.dom.dispatchEvent(new CustomEvent(SAVE_EVENT, { bubbles: true }));
            return true;
          },
        },
      ]),
      syntaxHighlighting(highlight),
      look,
      readOnlyPart.of(readOnlyExtension(readOnly)),
      languagePart.of([]),
      marksPart.of([]),
    ],
  });
}
