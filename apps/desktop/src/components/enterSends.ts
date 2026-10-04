/**
 * Enter sends what you write to an agent, as in Claude Code and the Chat (ADR-200): Enter sends,
 * Shift+Enter starts a new line, and Enter while a word is being put together (an IME, as for
 * Japanese or Chinese) only finishes the word. The boxes where you give an agent its work use it;
 * the boxes in forms (rules, servers, lessons, reports) keep Enter as a new line.
 */
import type { KeyboardEvent } from "react";

/** What a screen reader says of such a box (it is not shown). */
export const ENTER_SENDS = "Enter sends. Shift and Enter start a new line.";

/** Is this key press "send": Enter, not with Shift, and not finishing a word being put together? */
export function isSendKey(e: KeyboardEvent): boolean {
  return e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing;
}

/**
 * For a box in a form: Enter presses the form's own send button, and only when it can be pressed,
 * so an empty box, or an agent that is busy, sends nothing. Enter never adds a new line there;
 * Shift+Enter does.
 */
export function enterSends(e: KeyboardEvent<HTMLTextAreaElement>): void {
  if (!isSendKey(e)) return;
  e.preventDefault();
  const form = e.currentTarget.form;
  const send = form?.querySelector<HTMLButtonElement>('button[type="submit"]');
  if (form && send && !send.disabled) form.requestSubmit(send);
}
