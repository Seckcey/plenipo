import type { ReactNode } from "react";
import type { RemoteSettings } from "@plenipo/types";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { a11yProblems } from "../test/a11y";
import { DevicesSettings, PictureCode } from "./DevicesSettings";
import { describeRemoteEvent, minutesLeft } from "./words";
import { PhoneSwitch } from "./PhoneSwitch";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getRemote: vi.fn(),
    setRemoteSwitch: vi.fn(),
    startPhonePairing: vi.fn(),
    cancelPhonePairing: vi.fn(),
    answerPhonePairing: vi.fn(),
    renameDevice: vi.fn(),
    removeDevice: vi.fn(),
    unpauseDevice: vi.fn(),
    setKeptOnPc: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeRemote: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeLicense: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
const go = vi.fn();
const NOW = Date.UTC(2026, 9, 1, 17);

function inPage(part: ReactNode) {
  return render(
    <main>
      <h1>Settings</h1>
      <h2>Devices</h2>
      {part}
    </main>,
  );
}

function settings(
  patch: Partial<RemoteSettings> = {},
  remote: Partial<RemoteSettings["remote"]> = {},
): RemoteSettings {
  return {
    pro: true,
    comingSoon: false,
    page: "https://remote.getplenipo.com",
    pcName: "OFFICE-PC",
    sensitive: [
      { kind: "payment", label: "Money: buying, payments, refunds, payouts" },
      { kind: "dns", label: "Changing DNS" },
    ],
    ...patch,
    remote: {
      switchedOn: true,
      connected: true,
      devices: [],
      kept: { every: false, productionServers: false, kinds: [] },
      ...remote,
    },
  };
}

const qr = { size: 21, cells: "1".repeat(7) + "0".repeat(21 * 21 - 7) };

beforeEach(() => {
  vi.clearAllMocks();
  vi.spyOn(Date, "now").mockReturnValue(NOW);
});

describe("Settings → Devices", () => {
  it("adds a phone with a picture code or a typed code", async () => {
    api.getRemote.mockResolvedValue(settings());
    api.startPhonePairing.mockResolvedValue(
      settings(
        {},
        {
          pairing: {
            step: "showing",
            code: "7K3Q-M9TX-2HFD-R8WB",
            link: "https://remote.getplenipo.com/#pair=7K3QM9TX2HFDR8WB",
            qr,
            endsAt: NOW + 10 * 60_000,
            wrong: 0,
          },
        },
      ),
    );
    const user = userEvent.setup();
    const { container } = inPage(<DevicesSettings go={go} />);
    await user.click(await screen.findByRole("button", { name: "Add a phone" }));
    expect(api.startPhonePairing).toHaveBeenCalledOnce();
    expect(
      await screen.findByRole("img", { name: /Picture code \(QR code\)/ }),
    ).toBeInTheDocument();
    expect(screen.getByText("7K3Q-M9TX-2HFD-R8WB")).toBeInTheDocument();
    expect(screen.getByText(/It works once, for 10 minutes more/)).toBeInTheDocument();
    expect(screen.getByText(/add the page to your Home Screen/)).toBeInTheDocument();
    expect(a11yProblems(container)).toEqual([]);
  });

  it("asks Is this your phone?, and adds nothing until you say yes", async () => {
    api.getRemote.mockResolvedValue(
      settings(
        {},
        {
          pairing: {
            step: "asking",
            name: "Frank's iPhone",
            browser: "Safari on iPhone",
            since: NOW,
          },
        },
      ),
    );
    api.answerPhonePairing.mockResolvedValue(
      settings({}, { pairing: { step: "makingPasskey", name: "Frank's iPhone" } }),
    );
    const user = userEvent.setup();
    inPage(<DevicesSettings go={go} />);
    const dialog = await screen.findByRole("alertdialog", { name: "Is this your phone?" });
    expect(dialog).toHaveTextContent("Frank's iPhone");
    expect(dialog).toHaveTextContent("Safari on iPhone");
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(api.answerPhonePairing).toHaveBeenCalledWith(true);
    expect(await screen.findByText(/Finish on Frank's iPhone/)).toBeInTheDocument();
  });

  it("lists phones, and removes one after asking", async () => {
    const phone = {
      id: "AAAAAAAAAAAAAAAAAAAAAA",
      name: "Frank's iPhone",
      browser: "Safari on iPhone",
      addedAt: NOW - 86_400_000,
      lastSeenAt: NOW,
      paused: false,
      signedIn: true,
      notices: false,
    };
    api.getRemote.mockResolvedValue(settings({}, { devices: [phone] }));
    api.removeDevice.mockResolvedValue(settings());
    const user = userEvent.setup();
    inPage(<DevicesSettings go={go} />);
    expect(await screen.findByText("Frank's iPhone")).toBeInTheDocument();
    expect(screen.getByText("Signed in")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Remove" }));
    expect(api.removeDevice).not.toHaveBeenCalled();
    const ask = screen.getByRole("alertdialog", { name: "Remove Frank's iPhone?" });
    expect(ask).toHaveTextContent("It is cut off at once.");
    await user.click(screen.getAllByRole("button", { name: "Remove" })[0]!);
    expect(api.removeDevice).toHaveBeenCalledWith(phone.id);
    expect(await screen.findByText("No phones yet.")).toBeInTheDocument();
  });

  it("says a paused phone was paused, and un-pauses it", async () => {
    api.getRemote.mockResolvedValue(
      settings(
        {},
        {
          devices: [
            {
              id: "AAAAAAAAAAAAAAAAAAAAAA",
              name: "Phone",
              browser: "Chrome on Android",
              addedAt: NOW,
              lastSeenAt: null,
              paused: true,
              signedIn: false,
              notices: false,
            },
          ],
        },
      ),
    );
    api.unpauseDevice.mockResolvedValue(settings());
    const user = userEvent.setup();
    inPage(<DevicesSettings go={go} />);
    expect(await screen.findByText(/Paused after 3 failed checks/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Un-pause" }));
    expect(api.unpauseDevice).toHaveBeenCalledWith("AAAAAAAAAAAAAAAAAAAAAA");
  });

  it("keeps the approvals you tick on this PC, none to begin with", async () => {
    api.getRemote.mockResolvedValue(settings());
    api.setKeptOnPc.mockImplementation((kept) => Promise.resolve(settings({}, { kept })));
    const user = userEvent.setup();
    inPage(<DevicesSettings go={go} />);
    const money = await screen.findByRole("checkbox", {
      name: "Money: buying, payments, refunds, payouts",
    });
    expect(money).not.toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Every approval" })).not.toBeChecked();
    await user.click(money);
    expect(api.setKeptOnPc).toHaveBeenCalledWith({
      every: false,
      productionServers: false,
      kinds: ["payment"],
    });
    await waitFor(() => expect(money).toBeChecked());
  });

  it("on Free, says it is part of Pro and adds nothing", async () => {
    api.getRemote.mockResolvedValue(settings({ pro: false }, { switchedOn: false }));
    inPage(<DevicesSettings go={go} />);
    expect(await screen.findByText(/Part of Plenipo Pro\./)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add a phone" })).toBeDisabled();
  });

  it("before the relay is ready, says Coming soon", async () => {
    api.getRemote.mockResolvedValue(settings({ comingSoon: true }, { switchedOn: false }));
    inPage(<DevicesSettings go={go} />);
    expect(await screen.findByText(/Coming soon\./)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add a phone" })).toBeDisabled();
  });
});

describe("Use Plenipo from another device", () => {
  it("turns on and off", async () => {
    api.getRemote.mockResolvedValue(settings({}, { switchedOn: false }));
    api.setRemoteSwitch.mockResolvedValue(settings({}, { switchedOn: true }));
    const user = userEvent.setup();
    render(<PhoneSwitch />);
    const toggle = await screen.findByRole("switch", { name: "Use Plenipo from another device" });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    await user.click(toggle);
    expect(api.setRemoteSwitch).toHaveBeenCalledWith(true);
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "true"));
  });

  it("cannot be turned on on Free, or before the relay is ready", async () => {
    for (const patch of [{ pro: false }, { comingSoon: true }]) {
      api.getRemote.mockResolvedValue(settings(patch, { switchedOn: false }));
      const { unmount } = render(<PhoneSwitch />);
      const toggle = await screen.findByRole("switch", { name: "Use Plenipo from another device" });
      expect(toggle).toBeDisabled();
      unmount();
    }
  });
});

describe("the picture code", () => {
  it("draws its dark squares, with a quiet border", () => {
    const { container } = render(<PictureCode qr={qr} label="code" />);
    const svg = container.querySelector("svg")!;
    expect(svg.getAttribute("viewBox")).toBe("0 0 29 29");
    expect(container.querySelector("path")!.getAttribute("d")).toContain("M4 4h1v1h-1z");
  });

  it("says how long a code lasts in plain words", () => {
    expect(minutesLeft(NOW + 9.5 * 60_000, NOW)).toBe("9 minutes");
    expect(minutesLeft(NOW + 61_000, NOW)).toBe("1 minute");
    expect(minutesLeft(NOW + 30_000, NOW)).toBe("less than a minute");
  });
});

describe("Activity, for phones", () => {
  it("says which phone asked, and why Guard refused", () => {
    const phone = { device: "AAAA", name: "Frank's phone" };
    expect(describeRemoteEvent("remote.request", { ...phone, kind: "approve" })).toBe(
      "Frank's phone asked to approve",
    );
    expect(
      describeRemoteEvent("remote.request", { ...phone, kind: "refuse", fromNotice: true }),
    ).toBe("Frank's phone asked to refuse (from a notice)");
    expect(
      describeRemoteEvent("remote.refused", { ...phone, kind: "approve", why: "keptOnPc" }),
    ).toBe("Guard refused Frank's phone: approve (you keep that kind of approval on this PC)");
    expect(
      describeRemoteEvent("remote.refused", { ...phone, kind: "stop all", why: "copied" }),
    ).toBe("Guard refused Frank's phone: stop all (it was a copy of a request already made)");
  });

  it("says when phones are added, signed in, paused, and removed", () => {
    expect(
      describeRemoteEvent("remote.device_added", {
        name: "Frank's phone",
        browser: "Safari on iPhone",
      }),
    ).toBe("You added Frank's phone (Safari on iPhone)");
    expect(describeRemoteEvent("remote.signed_in", { name: "Frank's phone" })).toBe(
      "Frank's phone signed in",
    );
    expect(describeRemoteEvent("remote.signed_out", { name: "Frank's phone", why: "idle" })).toBe(
      "Frank's phone signed out after 30 minutes without use",
    );
    expect(describeRemoteEvent("remote.device_paused", { name: "Frank's phone" })).toBe(
      "Frank's phone was paused after 3 failed checks",
    );
    expect(describeRemoteEvent("remote.device_removed", { name: "Frank's phone", by: "pc" })).toBe(
      "You removed Frank's phone",
    );
    expect(
      describeRemoteEvent("remote.pairing_refused", { reason: "wrong_code", pairingPaused: true }),
    ).toBe("Wrong pairing codes were tried: Add a phone is paused for 15 minutes");
    expect(describeRemoteEvent("remote.switched_on", {})).toBe(
      "You turned on using Plenipo from another device",
    );
    expect(describeRemoteEvent("task.created", {})).toBeNull();
  });
});
