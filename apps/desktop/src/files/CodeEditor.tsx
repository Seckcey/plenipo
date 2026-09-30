import { useEffect, useLayoutEffect, useRef } from "react";
import { LanguageDescription } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { Transaction, type EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import {
  languagePart,
  marksExtension,
  marksPart,
  readOnlyExtension,
  readOnlyPart,
  type LineMark,
} from "./editorSetup";

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
  /** The state the page gives now: a replaced one is not reported back when its editor goes. */
  const given = useRef(state);
  useLayoutEffect(() => {
    changed.current = onChange;
    given.current = state;
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
      // Closing (the page goes): the page keeps what was typed. Replaced (Reload, a worker's
      // change read again): the old text is thrown away, never kept as "not saved".
      if (given.current === state) changed.current(edited.current ?? v.state);
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
      effects: readOnlyPart.reconfigure(readOnlyExtension(readOnly || shown !== null)),
    });
  }, [readOnly, shown, state]);

  useEffect(() => {
    view.current?.dispatch({ effects: marksPart.reconfigure(marksExtension(marks)) });
  }, [marks, state, shown]);

  return <div ref={host} className="code-editor" />;
}
