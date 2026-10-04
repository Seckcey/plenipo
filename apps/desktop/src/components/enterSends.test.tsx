import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { enterSends } from "./enterSends";

afterEach(cleanup);

/** A box where you write to an agent, in a form with its own send button. */
function Box({ onSend, canSend = true }: { onSend: (text: string) => void; canSend?: boolean }) {
  const [text, setText] = useState("");
  return (
    <form
      aria-label="Write"
      onSubmit={(e) => {
        e.preventDefault();
        onSend(text);
      }}
    >
      <textarea
        aria-label="Words"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={enterSends}
      />
      <button type="button">Attach</button>
      <button type="submit" disabled={!canSend}>
        Send
      </button>
    </form>
  );
}

describe("Enter sends (as in the Chat)", () => {
  it("sends with Enter, through the form's own send button, and adds no new line", async () => {
    const onSend = vi.fn();
    const user = userEvent.setup();
    render(<Box onSend={onSend} />);
    const box = screen.getByRole("textbox", { name: "Words" });
    await user.type(box, "Hello{Enter}");
    expect(onSend).toHaveBeenCalledExactlyOnceWith("Hello");
    expect(box).toHaveValue("Hello");
  });

  it("starts a new line with Shift+Enter, and sends nothing", async () => {
    const onSend = vi.fn();
    const user = userEvent.setup();
    render(<Box onSend={onSend} />);
    const box = screen.getByRole("textbox", { name: "Words" });
    await user.type(box, "One{Shift>}{Enter}{/Shift}two");
    expect(box).toHaveValue("One\ntwo");
    expect(onSend).not.toHaveBeenCalled();
  });

  it("only finishes the word while one is being put together (an IME)", () => {
    const onSend = vi.fn();
    render(<Box onSend={onSend} />);
    const box = screen.getByRole("textbox", { name: "Words" });
    // `false`: the key press was cancelled; `true`: the IME keeps it.
    expect(fireEvent.keyDown(box, { key: "Enter", isComposing: true })).toBe(true);
    expect(onSend).not.toHaveBeenCalled();
  });

  it("sends nothing while the send button cannot be pressed, and adds no new line", async () => {
    const onSend = vi.fn();
    const user = userEvent.setup();
    render(<Box onSend={onSend} canSend={false} />);
    const box = screen.getByRole("textbox", { name: "Words" });
    await user.type(box, "Busy{Enter}");
    expect(onSend).not.toHaveBeenCalled();
    expect(box).toHaveValue("Busy");
  });
});
