import { useEffect, useLayoutEffect, useRef } from "react";
import { basicSetup } from "codemirror";
import { indentWithTab } from "@codemirror/commands";
import { HighlightStyle, LanguageDescription, syntaxHighlighting } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import {
  Compartment,
  EditorState,
  RangeSetBuilder,
  Transaction,
  type Extension,
} from "@codemirror/state";
import { Decoration, EditorView, GutterMarker, gutter, keymap } from "@codemirror/view";
import { tags } from "@lezer/highlight";

/** How a line is marked while a worker's change shows (ADR-055 §4): a bar and a word. */
export type LineMark = "new" | "changed" | null;

/** The page hears this when Ctrl+S is pressed in the editor. */
export const SAVE_EVENT = "plenipo:save";

const readOnlyPart = new Compartment();
const languagePart = new Compartment();
const marksPart = new Compartment();

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
function marksExtension(marks: readonly LineMark[]): Extension {
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
      readOnlyPart.of(EditorState.readOnly.of(readOnly)),
      languagePart.of([]),
      marksPart.of([]),
    ],
  });
}

/**
 * The editor (CodeMirror 6, ADR-093 §5): code with colors for its language (found from the
 * file's name, loaded when first needed), Ctrl+S to save, and — while a worker's change shows —
 * its new and changed lines marked. `shown`: text to show instead of what is being edited (a
 * worker's change as it lands), read-only; the edited text comes back when it goes.
 */
export function CodeEditor({
  state,
  fileName,
  readOnly,
  shown,
  marks,
  label,
  onChange,
}: {
  /** The editor's contents to start from (a new file's, or one kept from before). */
  state: EditorState;
  fileName: string;
  readOnly: boolean;
  shown: string | null;
  marks: readonly LineMark[];
  label: string;
  /** The editor's contents after each change (and when it closes). */
  onChange: (state: EditorState) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  /** The edited contents, while `shown` covers them. */
  const edited = useRef<EditorState | null>(null);
  const changed = useRef(onChange);
  useLayoutEffect(() => {
    changed.current = onChange;
  });

  useEffect(() => {
    const parent = host.current;
    if (!parent) return;
    const v = new EditorView({
      state,
      parent,
      dispatchTransactions: (trs, v) => {
        v.update(trs);
        if (edited.current === null && trs.some((t) => t.docChanged || t.selection)) {
          changed.current(v.state);
        }
      },
    });
    v.contentDOM.setAttribute("aria-label", label);
    view.current = v;
    return () => {
      changed.current(edited.current ?? v.state);
      edited.current = null;
      v.destroy();
      view.current = null;
    };
    // A new state is a new file: the editor starts again (the rest update below).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state]);

  // A worker's change shows over the edited text (not in its undo history), and goes again.
  // First of the updates: the ones below set the language, read-only, and marks on whichever
  // contents are showing.
  useEffect(() => {
    const v = view.current;
    if (!v) return;
    if (shown !== null) {
      if (edited.current === null) edited.current = v.state;
      if (v.state.doc.toString() !== shown) {
        v.dispatch({
          changes: { from: 0, to: v.state.doc.length, insert: shown },
          annotations: Transaction.addToHistory.of(false),
        });
      }
    } else if (edited.current !== null) {
      const back = edited.current;
      edited.current = null;
      v.setState(back);
    }
  }, [shown, state]);

  // Its language, loaded when first needed.
  useEffect(() => {
    const v = view.current;
    if (!v) return;
    let live = true;
    const found = LanguageDescription.matchFilename(languages, fileName);
    if (!found) return;
    found
      .load()
      .then((support) => {
        if (live) v.dispatch({ effects: languagePart.reconfigure(support) });
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [fileName, state, shown]);

  useEffect(() => {
    view.current?.dispatch({
      effects: readOnlyPart.reconfigure(EditorState.readOnly.of(readOnly || shown !== null)),
    });
  }, [readOnly, shown, state]);

  useEffect(() => {
    view.current?.dispatch({ effects: marksPart.reconfigure(marksExtension(marks)) });
  }, [marks, state, shown]);

  return <div ref={host} className="code-editor" />;
}
