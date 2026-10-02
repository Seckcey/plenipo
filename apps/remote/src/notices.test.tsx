import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import type * as KeepModule from "./keep";
import { memoryKeep, type Kept } from "./keep";
import { encode } from "./lock/bytes";
import { newKeyPair } from "./lock/noise";
import { answerFromNotice } from "./notice-answer";
import { openedOn } from "./open-target";
import { FakePc } from "./test/fake-pc";

// The phone's keychain and its notice storage: stand-ins (jsdom has neither passkeys nor
// IndexedDB). The real ones are checked in the browser by the real-app test.
vi.mock("./lock/passkey", () => ({
  PasskeyProblem: class PasskeyProblem extends Error {},
  makePasskey: vi.fn(),
  answerChallenge: vi.fn(),
}));
const lockScreen = vi.hoisted(() => {
  const state: { choice: "show" | "hide" } = { choice: "show" };
  return state;
});
vi.mock("./keep", async (original) => ({
  ...(await original<typeof KeepModule>()),
  lockScreenChoice: vi.fn(() => Promise.resolve(lockScreen.choice)),
  setLockScreenChoice: vi.fn((c: "show" | "hide") => {
    lockScreen.choice = c;
    return Promise.resolve();
  }),
}));

const approval = {
  id: "a1",
  taskId: "t1",
  status: "pending",
  requestedAt: Date.now() - 10_000,
  expiresAt: Date.now() + 9 * 60_000,
  resolvedAt: null,
  worker: "Senior Developer",
  role: "Developer",
  capabilityLabel: "Run programs",
  summary: "git push origin",
  detail: "git push origin main",
  reason: "Publishing changes outside this computer.",
  riskLabel: "Sends outside",
  waiting: true,
};

let pc: FakePc;
let pending = true;

async function paired(): Promise<Kept> {
  const keys = await newKeyPair(true);
  pc.phones.set("cGhvbmUtMQ", keys.publicKey);
  return {
    keys,
    paired: {
      device: "ZGV2aWNlLTE",
      phone: "cGhvbmUtMQ",
      pass: "cGhvbmUtMQ.pass",
      pc: pc.fingerprint,
      pcKey: encode(pc.keys.publicKey),
      pcName: pc.pcName,
      credential: "Y3JlZGVudGlhbA",
    },
  };
}

beforeEach(async () => {
  localStorage.clear();
  lockScreen.choice = "show";
  pending = true;
  pc = await FakePc.start();
  pc.signedIn.add("cGhvbmUtMQ");
  pc.answer = (ask) => {
    switch (ask.kind) {
      case "readOrganizations":
        return { organizations: [{ id: "first", name: "8 West Ventures" }] };
      case "readHome":
        return { current: [], finished: [], stuck: [], going: 0, finishedDay: 0 };
      case "readControl":
        return { control: { stopped: false, sessions: [], revision: 1 }, recovery: [] };
      case "readApprovals":
        return [
          {
            org: "first",
            name: "8 West Ventures",
            queue: pending
              ? { pending: [approval], recent: [] }
              : {
                  pending: [],
                  recent: [
                    {
                      ...approval,
                      status: "approved",
                      note: "Approved by you, from Frank's phone.",
                    },
                  ],
                },
            keptOnPc: [],
          },
        ];
      case "refuse":
        pending = false;
        return { ...approval, status: "rejected" };
      case "readLessons":
        return { enabled: true, autoRoles: [], offRoles: [], agents: {}, waiting: [], kept: [] };
      case "readAiTools":
        return { tools: [], autoUpdate: false, lastLookedAt: null, looking: false };
      case "readDiagnostics":
        return { ledger: { taskCount: 0, eventCount: 0, lastBackup: null }, version: "1.19.2" };
      default:
        return {};
    }
  };
});

afterEach(() => {
  openedOn(null);
  vi.unstubAllGlobals();
});

describe("Refuse and Discard from a notice", () => {
  it("refuses an approval from its notice, in a meeting that may only say no", async () => {
    const answered = await answerFromNotice(
      { kind: "refuse", org: "first", approval: "a1" },
      memoryKeep(await paired()),
      pc.make,
    );
    expect(answered).toEqual({ ok: true });
    expect(pc.asked).toContainEqual({ kind: "refuse", org: "first", approval: "a1" });
  });

  it("says plainly when the PC can't be reached, and changes nothing", async () => {
    pc.online = false;
    const answered = await answerFromNotice(
      { kind: "discard", org: "first", lesson: "l1" },
      memoryKeep(await paired()),
      pc.make,
    );
    expect(answered).toEqual({
      ok: false,
      message: "Your PC can’t be reached. Nothing was changed.",
    });
    expect(pc.asked).toEqual([]);
  });

  it("says when the phone is no longer paired", async () => {
    const answered = await answerFromNotice(
      { kind: "refuse", org: "first", approval: "a1" },
      memoryKeep(null),
      pc.make,
    );
    expect(answered.ok).toBe(false);
  });
});

describe("a tapped notice opens its item, as it is now", () => {
  it("opens the approval it is about", async () => {
    openedOn("approval:first:a1");
    render(<App keep={memoryKeep(await paired())} make={pc.make} />);
    const card = (await screen.findByText("git push origin")).closest("li")!;
    expect(card).toHaveClass("approval--focused");
    expect(within(card).getByRole("button", { name: "Approve" })).toBeInTheDocument();
  });

  it("says an approval was already answered (an old notice cannot fool you)", async () => {
    pending = false;
    openedOn("approval:first:a1");
    render(<App keep={memoryKeep(await paired())} make={pc.make} />);
    expect(await screen.findByText("Already answered.")).toBeInTheDocument();
    expect(screen.getByRole("status", { name: "" })).toHaveTextContent(
      "Approved by you, from Frank's phone.",
    );
    expect(screen.queryByRole("button", { name: "Approve" })).toBeNull();
  });
});

describe("Notices on this phone", () => {
  function phoneWithNotices() {
    let current: PushSubscription | null = null;
    const unsubscribe = vi.fn(() => {
      current = null;
      return Promise.resolve(true);
    });
    const sub = {
      endpoint: "https://fcm.googleapis.com/fcm/send/phone-1",
      toJSON: () => ({
        endpoint: "https://fcm.googleapis.com/fcm/send/phone-1",
        keys: { p256dh: "BPhonePublicKey", auth: "PhoneSecret" },
      }),
      unsubscribe,
    } as unknown as PushSubscription;
    const pushManager = {
      getSubscription: vi.fn(() => Promise.resolve(current)),
      subscribe: vi.fn(() => {
        current = sub;
        return Promise.resolve(sub);
      }),
    };
    const registration = { pushManager };
    vi.stubGlobal("PushManager", function PushManager() {});
    vi.stubGlobal("Notification", {
      permission: "default",
      requestPermission: vi.fn(() => Promise.resolve("granted")),
    });
    Object.defineProperty(navigator, "serviceWorker", {
      configurable: true,
      value: {
        getRegistration: () => Promise.resolve(registration),
        ready: Promise.resolve(registration),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      },
    });
    return { pushManager, unsubscribe };
  }

  afterEach(() => {
    Reflect.deleteProperty(navigator, "serviceWorker");
  });

  it("signs up with the PC's notice key, and the PC gets this phone's notice address", async () => {
    const { pushManager, unsubscribe } = phoneWithNotices();
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await paired())} make={pc.make} />);
    await user.click(await screen.findByRole("button", { name: "More" }));
    const toggle = await screen.findByRole("switch", { name: "Notices on this phone" });
    await waitFor(() => expect(toggle).toBeEnabled());
    expect(toggle).toHaveAttribute("aria-checked", "false");
    await user.click(toggle);
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "true"));
    const [options] = pushManager.subscribe.mock.calls[0] as unknown as [
      { userVisibleOnly: boolean; applicationServerKey: ArrayBuffer },
    ];
    expect(options.userVisibleOnly).toBe(true);
    expect(new Uint8Array(options.applicationServerKey)).toEqual(new Uint8Array(65).fill(4));
    expect(pc.asked).toContainEqual({
      kind: "noticesOn",
      subscription: {
        endpoint: "https://fcm.googleapis.com/fcm/send/phone-1",
        p256dh: "BPhonePublicKey",
        auth: "PhoneSecret",
      },
    });
    // Off: the phone forgets its notice address, and so does the PC.
    await user.click(toggle);
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "false"));
    expect(unsubscribe).toHaveBeenCalled();
    expect(pc.asked).toContainEqual({ kind: "noticesOff" });
  });

  it("keeps the lock-screen choice on this phone", async () => {
    phoneWithNotices();
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await paired())} make={pc.make} />);
    await user.click(await screen.findByRole("button", { name: "More" }));
    const hide = await screen.findByLabelText("Show only “Something needs you”");
    expect(screen.getByLabelText("Show what it is")).toBeChecked();
    await user.click(hide);
    expect(hide).toBeChecked();
    expect(lockScreen.choice).toBe("hide");
  });

  it("asks for an update when the PC sends no notices yet", async () => {
    phoneWithNotices();
    pc.noticeKey = null;
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await paired())} make={pc.make} />);
    await user.click(await screen.findByRole("button", { name: "More" }));
    expect(
      await screen.findByText("Update Plenipo on your PC to get notices on this phone."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("switch", { name: "Notices on this phone" })).toBeNull();
  });

  it("on an iPhone, shows how to add Plenipo to the Home Screen first", async () => {
    phoneWithNotices();
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue(
      "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
    );
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await paired())} make={pc.make} />);
    await user.click(await screen.findByRole("button", { name: "More" }));
    expect(await screen.findByText("Add Plenipo to your Home Screen.")).toBeInTheDocument();
  });
});
