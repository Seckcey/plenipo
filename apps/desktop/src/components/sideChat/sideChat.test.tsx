import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { AgentSessionDetail } from "@plenipo/types";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as commands from "../../api/commands";
import { position } from "../../test/orgFixtures";
import { AskQuestionButton } from "./AskQuestion";

vi.mock("../../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return { ...actual, askSideQuestion: vi.fn() };
});

const api = vi.mocked(commands);

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("side chats (Phase 25, item 3.5)", () => {
  it("asks a full-time agent a question and opens the side chat", async () => {
    api.askSideQuestion.mockResolvedValue({
      session: { id: "side-1" },
    } as unknown as AgentSessionDetail);
    const onAsked = vi.fn();
    const user = userEvent.setup();
    const sup = position("sup", "Website Supervisor", "r-coord", null);
    render(<AskQuestionButton p={sup} onAsked={onAsked} />);
    await user.click(screen.getByRole("button", { name: "Ask a question" }));
    const dialog = screen.getByRole("dialog", { name: "Ask Website Supervisor a question" });
    expect(dialog).toHaveTextContent("it can't use tools or hand work to its team");
    const ask = within(dialog).getByRole("button", { name: "Ask" });
    expect(ask).toBeDisabled();
    await user.type(within(dialog).getByLabelText(/Your question/), "  How far along are you?  ");
    await user.click(ask);
    await waitFor(() => expect(onAsked).toHaveBeenCalledWith("side-1"));
    expect(api.askSideQuestion).toHaveBeenCalledWith("sup", "How far along are you?");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("asks with Enter; Shift+Enter starts a new line", async () => {
    api.askSideQuestion.mockResolvedValue({
      session: { id: "side-2" },
    } as unknown as AgentSessionDetail);
    const onAsked = vi.fn();
    const user = userEvent.setup();
    const sup = position("sup", "Website Supervisor", "r-coord", null);
    render(<AskQuestionButton p={sup} onAsked={onAsked} />);
    await user.click(screen.getByRole("button", { name: "Ask a question" }));
    const box = within(screen.getByRole("dialog")).getByLabelText(/Your question/);
    expect(box).toHaveAccessibleDescription("Enter sends. Shift and Enter start a new line.");
    // Nothing to ask yet: Enter does nothing, and adds no line.
    await user.type(box, "{Enter}");
    expect(box).toHaveValue("");
    await user.type(box, "How far{Shift>}{Enter}{/Shift}along?");
    // A word still being put together (an IME) is only finished.
    fireEvent.keyDown(box, { key: "Enter", isComposing: true });
    expect(api.askSideQuestion).not.toHaveBeenCalled();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(onAsked).toHaveBeenCalledWith("side-2"));
    expect(api.askSideQuestion).toHaveBeenCalledWith("sup", "How far\nalong?");
  });

  it("says why it could not ask, and has no button for an on-call position", async () => {
    api.askSideQuestion.mockRejectedValue(new Error("All work is stopped."));
    const user = userEvent.setup();
    const sup = position("sup", "Website Supervisor", "r-coord", null);
    const { rerender } = render(<AskQuestionButton p={sup} onAsked={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "Ask a question" }));
    const dialog = screen.getByRole("dialog");
    await user.type(within(dialog).getByLabelText(/Your question/), "Hi");
    await user.click(within(dialog).getByRole("button", { name: "Ask" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent("All work is stopped.");
    rerender(
      <AskQuestionButton
        p={position("dev", "Senior Developer", "r-dev", null)}
        onAsked={vi.fn()}
      />,
    );
    expect(screen.queryByRole("button", { name: "Ask a question" })).toBeNull();
  });
});
