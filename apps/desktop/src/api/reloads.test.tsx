import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import type { LedgerEvent } from "@plenipo/types";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useAiTools } from "../components/aiTools/useAiTools";
import { useApprovals, usePermissions } from "../guard/usePermissions";
import { useLedgerFeed } from "../ledger/useLedgerFeed";
import { useOrganization } from "../org/useOrganization";
import { useOrganizationNames } from "../org/useOrganizationNames";
import { useLive } from "../pages/useLive";
import { useProjectWork } from "../projects/useProjectWork";
import { useRouting } from "../routing/useRouting";
import { useConnections } from "../settings/connections/useConnections";
import { aiPage } from "../test/aiToolFixtures";
import { sampleCard, samplePage } from "../test/connectionFixtures";
import { sampleOrganization } from "../test/orgFixtures";
import { approval, samplePermissions, sampleQueue } from "../test/permissionFixtures";
import { sampleWork } from "../test/projectFixtures";
import { sampleRouting } from "../test/routingFixtures";
import * as commands from "./commands";

// Pages reload what Core says after each change, and Core answers each reload on its own thread,
// so answers can come back in any order. These tests answer them out of order on purpose: an
// older answer that comes back last must never put back what the page showed before.

vi.mock("./commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getApprovals: vi.fn(),
    getPermissions: vi.fn(),
    getConnections: vi.fn(),
    getOrganization: vi.fn(),
    getRouting: vi.fn(),
    getAiTools: vi.fn(),
    listTasks: vi.fn(),
    listRecentEvents: vi.fn(),
    getProjectWork: vi.fn(),
  };
});

const hub = vi.hoisted(() => ({ ledger: [] as ((event: unknown) => void)[] }));
vi.mock("./events", () => ({
  subscribeLedgerEvents: vi.fn((handler: (event: unknown) => void) => {
    hub.ledger.push(handler);
    return Promise.resolve(() => {
      hub.ledger = hub.ledger.filter((h) => h !== handler);
    });
  }),
  subscribeAgentUpdates: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeShared: vi.fn(() => Promise.resolve(() => undefined)),
}));

/** A Ledger event, to every page listening. */
function ledgerEvent(eventType: string, seq: number) {
  act(() => {
    for (const handler of hub.ledger) handler(event(eventType, seq));
  });
}

const event = (eventType: string, seq: number): LedgerEvent => ({
  seq,
  id: `e${seq}`,
  taskId: null,
  executionId: null,
  source: "owner",
  destination: null,
  eventType,
  payload: {},
  createdAt: seq,
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  hub.ledger = [];
});

interface Call<T> {
  resolve: (value: T) => void;
  reject: (reason: unknown) => void;
}

/** Each call of `command` waits until the test answers it, so answers can come in any order. */
function answerLater<T>(command: (...args: never[]) => Promise<T>): Call<T>[] {
  const calls: Call<T>[] = [];
  vi.mocked(command).mockImplementation(
    () => new Promise<T>((resolve, reject) => calls.push({ resolve, reject })),
  );
  return calls;
}

/** Answer `call`, and let the page take the answer in. */
async function answer<T>(call: Call<T> | undefined, value: T) {
  expect(call).toBeDefined();
  await act(() => {
    call?.resolve(value);
    return Promise.resolve();
  });
}

/** Fail `call`, and let the page take the failure in. */
async function fail<T>(call: Call<T> | undefined) {
  expect(call).toBeDefined();
  await act(() => {
    call?.reject("The Ledger could not be read.");
    return Promise.resolve();
  });
}

/** The first reload (as the page opens), then a second one: both under way. */
async function twoReloads<T>(calls: Call<T>[], reload: () => unknown) {
  await waitFor(() => expect(calls).toHaveLength(1));
  act(() => void reload());
  await waitFor(() => expect(calls).toHaveLength(2));
}

describe("the Approvals queue", () => {
  // Before: the queue was reloaded after each approval, and an answer from before the owner
  // decided could come back last, so a request the owner had already answered showed as waiting
  // again (with its Approve and Refuse buttons) until the next change.
  const waiting = sampleQueue();
  const answered = sampleQueue({ pending: [] });

  it("drops an older reload that answers after a newer one", async () => {
    const calls = answerLater(commands.getApprovals);
    const { result } = renderHook(() => useApprovals());
    await twoReloads(calls, () => result.current.reload());
    await answer(calls[1], answered);
    await answer(calls[0], waiting);
    expect(result.current.queue).toBe(answered);
  });

  it("keeps a change applied while a slower, older reload is under way", async () => {
    const calls = answerLater(commands.getApprovals);
    const { result } = renderHook(() => useApprovals());
    await waitFor(() => expect(calls).toHaveLength(1));
    act(() => result.current.apply(answered));
    await answer(calls[0], waiting);
    expect(result.current.queue).toBe(answered);
    // The reload that follows the change is newer, and shows.
    await waitFor(() => expect(calls).toHaveLength(2));
    const later = sampleQueue({ pending: [] });
    await answer(calls[1], later);
    expect(result.current.queue).toBe(later);
  });

  it("shows a request that arrived while the owner's answer was on its way", async () => {
    // The queue an answer returns is read before the answer travels back. A request that comes
    // in meanwhile sets off a reload that starts before the answer lands, so it is dropped as
    // older: the reload that follows every change brings the request in.
    const calls = answerLater(commands.getApprovals);
    const { result } = renderHook(() => useApprovals());
    await waitFor(() => expect(calls).toHaveLength(1));
    await answer(calls[0], waiting);
    act(() => void result.current.reload());
    await waitFor(() => expect(calls).toHaveLength(2));
    act(() => result.current.apply(answered));
    const newRequest = sampleQueue({
      pending: [approval({ id: "approval-2", summary: "run npm test" })],
    });
    await answer(calls[1], newRequest);
    expect(result.current.queue).toBe(answered);
    await waitFor(() => expect(calls).toHaveLength(3));
    const withIt = sampleQueue({
      pending: [approval({ id: "approval-2", summary: "run npm test" })],
    });
    await answer(calls[2], withIt);
    expect(result.current.queue).toBe(withIt);
  });

  it("does not let an older reload's failure cover a newer answer", async () => {
    const calls = answerLater(commands.getApprovals);
    const { result } = renderHook(() => useApprovals());
    await twoReloads(calls, () => result.current.reload());
    await answer(calls[1], answered);
    await fail(calls[0]);
    expect(result.current.queue).toBe(answered);
    expect(result.current.error).toBeNull();
  });

  it("shows an older answer that comes back after a newer failure, and clears it", async () => {
    // The page has just opened: nothing is shown yet.
    const calls = answerLater(commands.getApprovals);
    const { result } = renderHook(() => useApprovals());
    await twoReloads(calls, () => result.current.reload());
    await fail(calls[1]);
    expect(result.current.queue).toBeNull();
    expect(result.current.error).toBe("The Ledger could not be read.");
    await answer(calls[0], waiting);
    expect(result.current.queue).toBe(waiting);
    expect(result.current.error).toBeNull();
  });

  it("still shows a failure when it is the newest look", async () => {
    const calls = answerLater(commands.getApprovals);
    const { result } = renderHook(() => useApprovals());
    await twoReloads(calls, () => result.current.reload());
    await answer(calls[0], waiting);
    await fail(calls[1]);
    expect(result.current.queue).toBe(waiting);
    expect(result.current.error).toBe("The Ledger could not be read.");
  });
});

describe("Settings → Permissions", () => {
  // Before: a permission the owner had just changed could flip back on screen when an older
  // reload answered last, though Core had saved the change.
  it("drops an older reload that answers after a newer one", async () => {
    const calls = answerLater(commands.getPermissions);
    const { result } = renderHook(() => usePermissions());
    await twoReloads(calls, () => result.current.reload());
    const newer = samplePermissions();
    await answer(calls[1], newer);
    await answer(calls[0], samplePermissions());
    expect(result.current.snapshot).toBe(newer);
  });

  it("keeps a change applied while a slower, older reload is under way", async () => {
    const calls = answerLater(commands.getPermissions);
    const { result } = renderHook(() => usePermissions());
    await waitFor(() => expect(calls).toHaveLength(1));
    const changed = samplePermissions();
    act(() => result.current.apply(changed));
    await answer(calls[0], samplePermissions());
    expect(result.current.snapshot).toBe(changed);
  });
});

describe("Settings → Connections", () => {
  // Before: a role the owner had just added to a connection's "Who may use it" could vanish from
  // the card ("Nobody yet") when an older reload answered last, though Core had saved it.
  const before = samplePage(sampleCard());
  const after = samplePage(sampleCard({ mail: "fullAccess" }));

  it("drops an older reload that answers after a newer one", async () => {
    const calls = answerLater(commands.getConnections);
    const { result } = renderHook(() => useConnections());
    await twoReloads(calls, () => result.current.reload());
    await answer(calls[1], after);
    await answer(calls[0], before);
    expect(result.current.page).toBe(after);
  });

  it("keeps a change applied while a slower, older reload is under way", async () => {
    const calls = answerLater(commands.getConnections);
    const { result } = renderHook(() => useConnections());
    await waitFor(() => expect(calls).toHaveLength(1));
    act(() => result.current.apply(after));
    await answer(calls[0], before);
    expect(result.current.page).toBe(after);
  });

  it("does not let an older reload's failure cover a newer answer", async () => {
    const calls = answerLater(commands.getConnections);
    const { result } = renderHook(() => useConnections());
    await twoReloads(calls, () => result.current.reload());
    await answer(calls[1], after);
    await fail(calls[0]);
    expect(result.current.page).toBe(after);
    expect(result.current.error).toBeNull();
  });

  it("drops a sign-in look that started before a change and answers after it", async () => {
    const calls = answerLater(commands.getConnections);
    const { result } = renderHook(() => useConnections());
    await waitFor(() => expect(calls).toHaveLength(1));
    // A sign-in waits in the browser, so the page looks again every moment.
    await answer(calls[0], samplePage(sampleCard({}, { signingIn: true })));
    await waitFor(() => expect(calls).toHaveLength(2), { timeout: 4_000 });
    const changed = samplePage(sampleCard({ mail: "fullAccess" }, { signingIn: true }));
    act(() => result.current.apply(changed));
    await answer(calls[1], samplePage(sampleCard({}, { signingIn: true })));
    expect(result.current.page).toBe(changed);
  });
});

describe("the organization", () => {
  // Before: a tile the owner had just moved, hired, or archived could jump back on the canvas
  // and the organization chart when an older reload answered last.
  it("drops an older reload that answers after a newer one", async () => {
    const calls = answerLater(commands.getOrganization);
    const { result } = renderHook(() => useOrganization());
    await twoReloads(calls, () => result.current.reload());
    const newer = sampleOrganization();
    await answer(calls[1], newer);
    await answer(calls[0], sampleOrganization());
    expect(result.current.snapshot).toBe(newer);
  });

  it("keeps a change applied while a slower, older reload is under way", async () => {
    const calls = answerLater(commands.getOrganization);
    const { result } = renderHook(() => useOrganization());
    await waitFor(() => expect(calls).toHaveLength(1));
    const moved = sampleOrganization();
    act(() => result.current.apply(moved));
    await answer(calls[0], sampleOrganization());
    expect(result.current.snapshot).toBe(moved);
    // The reload that follows the change is newer, and shows.
    await waitFor(() => expect(calls).toHaveLength(2));
    const later = sampleOrganization();
    await answer(calls[1], later);
    expect(result.current.snapshot).toBe(later);
  });

  it("does not let an older reload's failure cover a newer answer", async () => {
    const calls = answerLater(commands.getOrganization);
    const { result } = renderHook(() => useOrganization());
    await twoReloads(calls, () => result.current.reload());
    const newer = sampleOrganization();
    await answer(calls[1], newer);
    await fail(calls[0]);
    expect(result.current.snapshot).toBe(newer);
    expect(result.current.status).toBe("ready");
    expect(result.current.error).toBeNull();
  });

  it("shows an older answer that comes back after a newer failure, and clears it", async () => {
    // The canvas has just opened: nothing is shown yet.
    const calls = answerLater(commands.getOrganization);
    const { result } = renderHook(() => useOrganization());
    await twoReloads(calls, () => result.current.reload());
    await fail(calls[1]);
    expect(result.current.status).toBe("error");
    const older = sampleOrganization();
    await answer(calls[0], older);
    expect(result.current.snapshot).toBe(older);
    expect(result.current.status).toBe("ready");
    expect(result.current.error).toBeNull();
  });
});

describe("the top bar's Showing picker", () => {
  // Before: a department or project just added, renamed, or archived could show as it was before
  // in the picker, until the organization changed again.
  it("drops an older load that answers after a newer one", async () => {
    const calls = answerLater(commands.getOrganization);
    const { result } = renderHook(() => useOrganizationNames());
    await waitFor(() => expect(calls).toHaveLength(1));
    // It loads again a moment after the organization changes.
    ledgerEvent("org.project_created", 1);
    await waitFor(() => expect(calls).toHaveLength(2));
    const newer = sampleOrganization();
    await answer(calls[1], newer);
    await answer(calls[0], sampleOrganization());
    expect(result.current.snapshot).toBe(newer);
  });
});

describe("Settings → AI models", () => {
  // Before: a model rule just changed could flip back on screen, though it was saved.
  it("drops an older reload that answers after a newer one", async () => {
    const calls = answerLater(commands.getRouting);
    const { result } = renderHook(() => useRouting());
    await twoReloads(calls, () => result.current.reload());
    const newer = sampleRouting();
    await answer(calls[1], newer);
    await answer(calls[0], sampleRouting());
    expect(result.current.snapshot).toBe(newer);
  });

  it("keeps a change applied while a slower, older reload is under way", async () => {
    const calls = answerLater(commands.getRouting);
    const { result } = renderHook(() => useRouting());
    await waitFor(() => expect(calls).toHaveLength(1));
    const changed = sampleRouting();
    act(() => result.current.apply(changed));
    await answer(calls[0], sampleRouting());
    expect(result.current.snapshot).toBe(changed);
    // The reload that follows the change is newer, and shows.
    await waitFor(() => expect(calls).toHaveLength(2));
    const later = sampleRouting();
    await answer(calls[1], later);
    expect(result.current.snapshot).toBe(later);
  });

  it("does not let an older reload's failure cover a newer answer", async () => {
    const calls = answerLater(commands.getRouting);
    const { result } = renderHook(() => useRouting());
    await twoReloads(calls, () => result.current.reload());
    const newer = sampleRouting();
    await answer(calls[1], newer);
    await fail(calls[0]);
    expect(result.current.snapshot).toBe(newer);
    expect(result.current.error).toBeNull();
  });
});

describe("Settings → AI tools", () => {
  // Before: an AI tool's state (updating, signed in, turned on) could go back to what it was a
  // moment earlier, though Plenipo had moved on.
  it("drops an older reload that answers after a newer one", async () => {
    const calls = answerLater(commands.getAiTools);
    const { result } = renderHook(() => useAiTools());
    await twoReloads(calls, () => result.current.reload());
    const newer = aiPage([]);
    await answer(calls[1], newer);
    await answer(calls[0], aiPage([]));
    expect(result.current.page).toBe(newer);
  });

  it("keeps a change applied while a slower, older reload is under way", async () => {
    const calls = answerLater(commands.getAiTools);
    const { result } = renderHook(() => useAiTools());
    await waitFor(() => expect(calls).toHaveLength(1));
    const changed = aiPage([]);
    act(() => result.current.apply(changed));
    await answer(calls[0], aiPage([]));
    expect(result.current.page).toBe(changed);
    // The reload that follows the change is newer, and shows.
    await waitFor(() => expect(calls).toHaveLength(2));
    const later = aiPage([]);
    await answer(calls[1], later);
    expect(result.current.page).toBe(later);
  });

  it("does not let an older reload's failure cover a newer answer", async () => {
    const calls = answerLater(commands.getAiTools);
    const { result } = renderHook(() => useAiTools());
    await twoReloads(calls, () => result.current.reload());
    const newer = aiPage([]);
    await answer(calls[1], newer);
    await fail(calls[0]);
    expect(result.current.page).toBe(newer);
    expect(result.current.error).toBeNull();
  });
});

describe("a department's, project's, worker's, or task's page", () => {
  // Before: two reloads of the same page could cross, and the page then showed what was true a
  // moment earlier (a task's state, its approvals) until the next change.
  it("drops an older reload that answers after a newer one", async () => {
    const calls: Call<string>[] = [];
    const load = vi.fn(
      () => new Promise<string>((resolve, reject) => calls.push({ resolve, reject })),
    );
    const { result } = renderHook(() => useLive("p1", load, () => true));
    await twoReloads(calls, () => result.current.reload());
    await answer(calls[1], "newer");
    await answer(calls[0], "older");
    expect(result.current.value).toBe("newer");
  });

  it("does not let an older reload's failure cover a newer answer", async () => {
    const calls: Call<string>[] = [];
    const load = vi.fn(
      () => new Promise<string>((resolve, reject) => calls.push({ resolve, reject })),
    );
    const { result } = renderHook(() => useLive("p1", load, () => true));
    await twoReloads(calls, () => result.current.reload());
    await answer(calls[1], "newer");
    await fail(calls[0]);
    expect(result.current.value).toBe("newer");
    expect(result.current.error).toBeNull();
  });
});

describe("a project's work", () => {
  // Before: a working copy just removed, or an objective's change just kept, could show as it was
  // before when an older reload answered last.
  it("drops an older reload that answers after a newer one", async () => {
    const calls = answerLater(commands.getProjectWork);
    const { result } = renderHook(() => useProjectWork("p1"));
    await twoReloads(calls, () => result.current.reload());
    const newer = sampleWork();
    await answer(calls[1], newer);
    await answer(calls[0], sampleWork());
    expect(result.current.work).toBe(newer);
  });

  it("keeps a change applied while a slower, older reload is under way", async () => {
    const calls = answerLater(commands.getProjectWork);
    const { result } = renderHook(() => useProjectWork("p1"));
    await waitFor(() => expect(calls).toHaveLength(1));
    const changed = sampleWork();
    act(() => result.current.apply(changed));
    await answer(calls[0], sampleWork());
    expect(result.current.work).toBe(changed);
    // The reload that follows the change is newer, and shows.
    await waitFor(() => expect(calls).toHaveLength(2));
    const later = sampleWork();
    await answer(calls[1], later);
    expect(result.current.work).toBe(later);
  });

  it("does not let an older reload's failure cover a newer answer", async () => {
    const calls = answerLater(commands.getProjectWork);
    const { result } = renderHook(() => useProjectWork("p1"));
    await twoReloads(calls, () => result.current.reload());
    const newer = sampleWork();
    await answer(calls[1], newer);
    await fail(calls[0]);
    expect(result.current.work).toBe(newer);
    expect(result.current.error).toBeNull();
  });
});

describe("Activity's feed", () => {
  // Before: the newest events, and a task's new state, could disappear from Activity until the
  // next event, when an older reload answered last.
  it("drops an older reload that answers after a newer one", async () => {
    const tasks = answerLater(commands.listTasks);
    const events = answerLater(commands.listRecentEvents);
    const { result } = renderHook(() => useLedgerFeed());
    await twoReloads(events, () => result.current.reload());
    const newer = [event("task.created", 2), event("task.created", 1)];
    await answer(tasks[1], []);
    await answer(events[1], newer);
    await answer(tasks[0], []);
    await answer(events[0], [event("task.created", 1)]);
    expect(result.current.events).toBe(newer);
  });

  it("does not let an older reload's failure cover a newer answer", async () => {
    const tasks = answerLater(commands.listTasks);
    const events = answerLater(commands.listRecentEvents);
    const { result } = renderHook(() => useLedgerFeed());
    await twoReloads(events, () => result.current.reload());
    const newer = [event("task.created", 1)];
    await answer(tasks[1], []);
    await answer(events[1], newer);
    await fail(tasks[0]);
    expect(result.current.events).toBe(newer);
    expect(result.current.status).toBe("ready");
    expect(result.current.error).toBeNull();
  });
});
