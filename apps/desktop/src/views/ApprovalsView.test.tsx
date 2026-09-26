import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { LedgerEvent } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import * as events from "../api/events";
import { approval, samplePermissions, sampleQueue, T0 } from "../test/permissionFixtures";
import { ApprovalsView } from "./ApprovalsView";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getApprovals: vi.fn(),
    getPermissions: vi.fn(),
    resolveApproval: vi.fn(),
    revokeGrant: vi.fn(),
  };
});
vi.mock("../api/events", () => ({ subscribeLedgerEvents: vi.fn() }));

const api = vi.mocked(commands);
let handlers: ((event: LedgerEvent) => void)[] = [];
const emit = (event: LedgerEvent) => handlers.forEach((h) => h(event));

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  vi.setSystemTime(T0);
  api.getApprovals.mockResolvedValue(sampleQueue());
  api.getPermissions.mockResolvedValue(samplePermissions());
  api.resolveApproval.mockResolvedValue(sampleQueue({ pending: [] }));
  api.revokeGrant.mockResolvedValue(samplePermissions({ grants: [] }));
  handlers = [];
  vi.mocked(events.subscribeLedgerEvents).mockImplementation((handler) => {
    handlers.push(handler);
    return Promise.resolve(() => undefined);
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe("Approvals", () => {
  it("shows a clear approval card and waits for the owner's answer", async () => {
    render(<ApprovalsView />);
    const card = await screen.findByRole("article", {
      name: "Backend Developer wants to git push origin",
    });
    expect(
      within(card).getByText("Senior Developer · Website project", { exact: false }),
    ).toBeInTheDocument();
    expect(within(card).getByLabelText("Exactly what it will do")).toHaveTextContent(
      "git push origin",
    );
    expect(within(card).getByText(/it sends commits to a server/)).toBeInTheDocument();
    expect(
      within(card).getByText("Sending or publishing outside this computer"),
    ).toBeInTheDocument();
    expect(within(card).getByText("Reaches outside this computer")).toBeInTheDocument();
    expect(within(card).getByText("9 min left")).toBeInTheDocument();
    await userEvent
      .setup({ advanceTimers: vi.advanceTimersByTime })
      .click(within(card).getByRole("button", { name: "Approve" }));
    expect(api.resolveApproval).toHaveBeenCalledWith("approval-1", true);
    expect(await screen.findByText("Nothing is waiting for your approval.")).toBeInTheDocument();
  });

  it("denies, and shows recent answers", async () => {
    render(<ApprovalsView />);
    const card = await screen.findByRole("article");
    await userEvent
      .setup({ advanceTimers: vi.advanceTimersByTime })
      .click(within(card).getByRole("button", { name: "Deny" }));
    expect(api.resolveApproval).toHaveBeenCalledWith("approval-1", false);
    const answers = screen.getByRole("heading", { name: "Recent answers" }).parentElement!;
    expect(within(answers).getByText("Not approved")).toBeInTheDocument();
    expect(within(answers).getByText(/run npm publish/)).toBeInTheDocument();
  });

  it("shows a refusal, and a request no worker waits for any more", async () => {
    api.getApprovals.mockResolvedValue(sampleQueue({ pending: [approval({ waiting: false })] }));
    api.resolveApproval.mockRejectedValue({
      kind: "invalidInput",
      message: "that request expired before your answer",
    });
    render(<ApprovalsView />);
    const card = await screen.findByRole("article");
    expect(within(card).getByText(/No worker is waiting for this answer/)).toBeInTheDocument();
    await userEvent
      .setup({ advanceTimers: vi.advanceTimersByTime })
      .click(within(card).getByRole("button", { name: "Approve" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("expired before your answer");
  });

  it("lists workers using permissions and revokes after a confirmation", async () => {
    const onOpenTask = vi.fn();
    render(<ApprovalsView onOpenTask={onOpenTask} />);
    const row = await screen.findByRole("row", { name: /^Backend Developer/ });
    expect(within(row).getByText("Read files")).toBeInTheDocument();
    expect(within(row).getByText("Run PowerShell scripts (asks)")).toBeInTheDocument();
    expect(within(row).getByText(/3 done · 1 blocked · 1 asked/)).toBeInTheDocument();
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    await user.click(
      within(row).getByRole("button", { name: "Revoke Backend Developer's permissions" }),
    );
    expect(api.revokeGrant).not.toHaveBeenCalled();
    await user.click(within(row).getByRole("button", { name: "Revoke now" }));
    expect(api.revokeGrant).toHaveBeenCalledWith("grant-1");
    expect(
      await screen.findByText("No worker is using permissions right now."),
    ).toBeInTheDocument();
  });

  it("makes blocked requests visible and links to their task", async () => {
    const onOpenTask = vi.fn();
    render(<ApprovalsView onOpenTask={onOpenTask} />);
    const blocked = (await screen.findByRole("heading", { name: "Recently blocked" }))
      .parentElement!;
    expect(within(blocked).getByText("read ../outside.txt", { exact: false })).toBeInTheDocument();
    expect(within(blocked).getByText(/is outside the project folder/)).toBeInTheDocument();
    await userEvent
      .setup({ advanceTimers: vi.advanceTimersByTime })
      .click(within(blocked).getByRole("button", { name: "Open task" }));
    expect(onOpenTask).toHaveBeenCalledWith("task-7");
  });

  it("reloads when an approval is requested", async () => {
    api.getApprovals.mockResolvedValueOnce(sampleQueue({ pending: [], recent: [] }));
    render(<ApprovalsView />);
    expect(await screen.findByText("Nothing is waiting for your approval.")).toBeInTheDocument();
    emit({
      seq: 1,
      id: "e",
      taskId: "task-7",
      executionId: null,
      source: "agent:codex",
      destination: null,
      eventType: "approval.requested",
      payload: {},
      createdAt: T0,
    });
    await vi.advanceTimersByTimeAsync(200);
    expect(await screen.findByRole("article")).toBeInTheDocument();
  });
});
