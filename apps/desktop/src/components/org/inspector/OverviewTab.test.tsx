import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ChatContext, type ChatApi } from "../../../chat/context";
import { TerminalContext, type TerminalApi } from "../../../terminal/context";
import { sampleOrganization } from "../../../test/orgFixtures";
import { OverviewTab } from "./OverviewTab";
import type { InspectorActions } from "./types";

const actions = { openTask: vi.fn(), openSession: vi.fn() } as unknown as InspectorActions;

function show(id: string, openWatch?: TerminalApi["openWatch"]) {
  const snapshot = sampleOrganization();
  const p = snapshot.positions.find((x) => x.id === id)!;
  const tab = <OverviewTab p={p} snapshot={snapshot} actions={actions} onSelect={vi.fn()} />;
  const wrap = (children: ReactNode) =>
    openWatch ? (
      <TerminalContext.Provider value={{ openWatch } as unknown as TerminalApi}>
        {children}
      </TerminalContext.Provider>
    ) : (
      children
    );
  render(wrap(tab));
}

afterEach(cleanup);

describe("the Overview tab's Watch button", () => {
  it("opens the agent's Watch tab in the terminal panel", async () => {
    const user = userEvent.setup();
    const openWatch = vi.fn();
    show("p-dev", openWatch);
    const watch = screen.getByRole("button", { name: "Watch Senior Developer" });
    expect(watch).toHaveTextContent("Watch");
    expect(watch).toHaveAccessibleDescription(
      "See the code it writes as it writes it, in the terminal panel. Read-only.",
    );
    await user.click(watch);
    expect(openWatch).toHaveBeenCalledWith("p-dev", "Senior Developer");
  });

  it("is not there without a terminal panel, nor for a vacant position", () => {
    show("p-dev");
    expect(screen.queryByRole("button", { name: /^Watch/ })).toBeNull();
    cleanup();
    show("p-mkt", vi.fn());
    expect(screen.queryByRole("button", { name: /^Watch/ })).toBeNull();
  });
});

describe("the Overview tab's chat (ADR-200, ADR-203)", () => {
  function withChat(canPopOut: boolean) {
    const chat = { open: vi.fn(), openWindow: vi.fn(), canPopOut } as unknown as ChatApi;
    const snapshot = sampleOrganization();
    const p = snapshot.positions.find((x) => x.id === "p-eng")!;
    render(
      <ChatContext.Provider value={chat}>
        <OverviewTab p={p} snapshot={snapshot} actions={actions} onSelect={vi.fn()} />
      </ChatContext.Provider>,
    );
    const target = { positionId: p.id, sessionId: p.agent?.sessionId ?? null, title: p.title };
    return { chat, target, title: p.title };
  }

  it("opens its chat in the Chat panel, or in a window of its own", async () => {
    const user = userEvent.setup();
    const { chat, target, title } = withChat(true);
    const alone = screen.getByRole("button", { name: `Pop out ${title}'s chat` });
    expect(alone).toHaveAccessibleDescription(
      "The same chat in a window of its own, beside your work. Up to six at once.",
    );
    await user.click(alone);
    expect(chat.openWindow).toHaveBeenCalledWith(target);
    await user.click(screen.getByRole("button", { name: `Chat with ${title}` }));
    expect(chat.open).toHaveBeenCalledWith(target);
  });

  it("offers no window of its own where chats cannot have one", () => {
    const { title } = withChat(false);
    expect(screen.getByRole("button", { name: `Chat with ${title}` })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: `Pop out ${title}'s chat` })).toBeNull();
  });
});
