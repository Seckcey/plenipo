/**
 * Enter sends what you write to an agent, as in Claude Code and the Chat (ADR-200): Enter sends,
 * Shift+Enter starts a new line, and Enter while a word is being put together (an IME, as for
 * Japanese or Chinese) only finishes the word. The boxes where you give an agent its work use it;
 * the boxes in forms (rules, servers, lessons, reports) keep Enter as a new line.
 */
import type { KeyboardEvent } from "react";

/** What a screen reader says of such a box (it is not shown). */
export const ENTER_SENDS = "Enter sends. Shift and Enter start a new line.";

/** The key code of a key press an IME is still putting together. */
const COMPOSING_KEY = 229;

/**
 * Is this key press "send": Enter, not with Shift, and not finishing a word being put together?
 * Chromium (Windows) says the word is still being put together with `isComposing`. WebKit (Mac,
 * Linux) sends the Enter that finishes the word just after, with `isComposing` false and key code
 * 229, the sign Lexical, ProseMirror, and Slate read too.
 */
export function isSendKey(e: KeyboardEvent): boolean {
  const composing = e.nativeEvent.isComposing || e.nativeEvent.keyCode === COMPOSING_KEY;
  return e.key === "Enter" && !e.shiftKey && !composing;
}

/**
 * For a box in a form: Enter presses the form's own send button, and only when it can be pressed,
 * so an empty box, or an agent that is busy, sends nothing. Enter then never adds a new line;
 * Shift+Enter does. A box with no form or no send button keeps Enter as a new line.
 */
export function enterSends(e: KeyboardEvent<HTMLTextAreaElement>): void {
  if (!isSendKey(e)) return;
  const form = e.currentTarget.form;
  const send = form?.querySelector<HTMLButtonElement>('button[type="submit"]');
  if (!form || !send) return;
  e.preventDefault();
  if (!send.disabled) form.requestSubmit(send);
}
