import { beforeEach, describe, expect, it, vi } from "vitest";

const fake = vi.hoisted(() => ({
  listen: vi.fn(() => Promise.resolve(() => undefined)),
  /** This window's label; none: not in a Plenipo window. */
  labels: ["org-0123456789abcdef0123456789abcdef"],
}));

vi.mock("@tauri-apps/api/event", () => ({ listen: fake.listen }));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => {
    const [label] = fake.labels;
    if (label === undefined) throw new Error("not in a window");
    return { label };
  },
}));
const listen = fake.listen;

import {
  COMMUNITY_MESSAGES_EVENT,
  LEDGER_EVENT,
  subscribeCommunityMessages,
  subscribeLedgerEvents,
  subscribeOrganizations,
} from "./events";

beforeEach(() => {
  listen.mockClear();
});

describe("a window hears its own organization's updates only (Phase 21, ADR-094 §12)", () => {
  it("listens for events sent to this window, never to every window", async () => {
    await subscribeLedgerEvents(() => undefined);
    expect(listen).toHaveBeenCalledWith(LEDGER_EVENT, expect.any(Function), {
      target: { kind: "WebviewWindow", label: "org-0123456789abcdef0123456789abcdef" },
    });
    await subscribeOrganizations(() => undefined);
    expect(listen).toHaveBeenLastCalledWith("plenipo://organizations", expect.any(Function), {
      target: { kind: "WebviewWindow", label: "org-0123456789abcdef0123456789abcdef" },
    });
  });

  it("outside a Plenipo window (tests), it listens as before", async () => {
    fake.labels.length = 0;
    await subscribeLedgerEvents(() => undefined);
    expect(listen).toHaveBeenCalledWith(LEDGER_EVENT, expect.any(Function));
  });
});

describe("Community messages (Phase 24)", () => {
  it("listens for plenipo://community-messages, for this window, and calls the handler", async () => {
    fake.labels.push("org-0123456789abcdef0123456789abcdef");
    const handler = vi.fn();
    await subscribeCommunityMessages(handler);
    expect(COMMUNITY_MESSAGES_EVENT).toBe("plenipo://community-messages");
    expect(listen).toHaveBeenCalledWith("plenipo://community-messages", expect.any(Function), {
      target: { kind: "WebviewWindow", label: "org-0123456789abcdef0123456789abcdef" },
    });
    const call = listen.mock.calls[0] as unknown as [string, (event: { payload: string }) => void];
    call[1]({ payload: "changed" });
    expect(handler).toHaveBeenCalledTimes(1);
  });
});
