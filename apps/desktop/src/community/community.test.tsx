import type { ReactNode } from "react";
import { SYSTEM_WORDS, type CommunityView, type Stage } from "@plenipo/types";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { SETTINGS_SECTIONS } from "../settings/sections";
import { setSystemWords } from "../system/words";
import { a11yProblems } from "../test/a11y";
import { CommunitySettings } from "./CommunitySettings";
import { CommunitySwitch } from "./CommunitySwitch";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getCommunity: vi.fn(),
    setCommunitySwitch: vi.fn(),
    checkCommunityAgain: vi.fn(),
    cancelCommunitySignIn: vi.fn(),
    joinCommunity: vi.fn(),
    signOutOfCommunity: vi.fn(),
    openCommunityPage: vi.fn(),
  };
});
// The handler the page gave to listen for changes: calling it is "Community changed".
const changed: { current: () => void } = { current: () => undefined };
vi.mock("../api/events", () => ({
  subscribeCommunity: vi.fn((handler: () => void) => {
    changed.current = handler;
    return Promise.resolve(() => undefined);
  }),
}));

const api = vi.mocked(commands);
const go = vi.fn();

/** Community as the app gives it: off to begin with, and nothing about anyone yet. */
function community(patch: Partial<CommunityView> = {}): CommunityView {
  return {
    stage: "off",
    switchedOn: false,
    comingSoon: false,
    code: null,
    codeExpiresAt: null,
    accountName: null,
    terms: null,
    member: null,
    pro: false,
    linksOpen: false,
    collaboratorsOpen: false,
    problem: null,
    ...patch,
  };
}

const signingIn = community({ stage: "signingIn", code: "4KQ-7TD", codeExpiresAt: 1_790_000_000n });
const joining = community({
  stage: "joining",
  accountName: "Frank Gonzalez",
  terms: "2026-10-01",
});
const member = {
  name: "pat-lee",
  adult: true,
  canStart: false,
  standing: "ok",
  pausedUntil: null,
  appearOffline: false,
};
const signedIn = community({
  stage: "signedIn",
  switchedOn: true,
  accountName: "Frank Gonzalez",
  member,
});

function inPage(part: ReactNode) {
  return render(
    <main>
      <h1>Settings</h1>
      <h2>Community</h2>
      {part}
    </main>,
  );
}

function theSwitch() {
  return screen.findByRole("switch", { name: "Community" });
}

/** Fill in the whole join form (the name, the month, the year, and the box). */
async function fillIn(user: ReturnType<typeof userEvent.setup>, name: string, year: string) {
  await user.type(screen.getByRole("textbox", { name: "Your name in Community" }), name);
  await user.selectOptions(screen.getByRole("combobox", { name: "Birth month" }), "April");
  await user.selectOptions(screen.getByRole("combobox", { name: "Birth year" }), year);
  await user.click(screen.getByRole("checkbox", { name: "I agree to the Community terms" }));
}

beforeEach(() => {
  vi.clearAllMocks();
  changed.current = () => undefined;
});

afterEach(() => setSystemWords(SYSTEM_WORDS.windows));

describe("Settings → Switches → Community", () => {
  const hints: [Stage, Partial<CommunityView>, boolean, string][] = [
    [
      "off",
      {},
      false,
      "Off to start with. On: find other Plenipo owners, message them, and work together, through your 8 West account. Plenipo asks 8 West whether Community is open only when you press this.",
    ],
    ["comingSoon", { comingSoon: true }, false, "Coming soon: 8 West hasn't opened Community yet."],
    ["updateNeeded", {}, false, "Update Plenipo to use Community (Settings → Updates)."],
    [
      "unreachable",
      { problem: "8 West's account site is busy. Nothing was changed." },
      false,
      "8 West's account site is busy. Nothing was changed.",
    ],
    ["unreachable", {}, false, "Community can't be reached right now. Nothing was changed."],
    ["signingIn", { code: "4KQ-7TD" }, true, "Signing in: finish in Settings → Community."],
    [
      "joining",
      { accountName: "Frank Gonzalez" },
      true,
      "Almost there: finish joining in Settings → Community.",
    ],
    [
      "signedIn",
      { switchedOn: true, accountName: "Frank Gonzalez" },
      true,
      "On. Signed in as Frank Gonzalez.",
    ],
    [
      "signedOut",
      { switchedOn: true },
      true,
      "On, but this PC is signed out. Sign in from Settings → Community.",
    ],
    [
      "closed",
      { switchedOn: true, accountName: "Frank Gonzalez" },
      true,
      "Community is closed for now. Everything on this PC is kept.",
    ],
  ];

  it.each(hints)("says what %s means, and whether it is on", async (stage, patch, on, hint) => {
    api.getCommunity.mockResolvedValue(community({ stage, ...patch }));
    render(<CommunitySwitch />);
    const toggle = await theSwitch();
    expect(toggle).toHaveAttribute("aria-checked", String(on));
    expect(toggle).toHaveAccessibleDescription(expect.stringContaining(hint));
  });

  it("uses this computer's own words for itself", async () => {
    setSystemWords(SYSTEM_WORDS.mac);
    api.getCommunity.mockResolvedValue(community({ stage: "signedOut", switchedOn: true }));
    render(<CommunitySwitch />);
    expect(await theSwitch()).toHaveAccessibleDescription(
      "On, but this Mac is signed out. Sign in from Settings → Community.",
    );
  });

  it("turns on by asking 8 West, and then shows that it is signing in", async () => {
    api.getCommunity.mockResolvedValue(community());
    api.setCommunitySwitch.mockResolvedValue(signingIn);
    const user = userEvent.setup();
    render(<CommunitySwitch />);
    const toggle = await theSwitch();
    expect(toggle).toHaveAttribute("aria-checked", "false");
    // Nothing is asked of 8 West until it is pressed.
    expect(api.setCommunitySwitch).not.toHaveBeenCalled();
    await user.click(toggle);
    expect(api.setCommunitySwitch).toHaveBeenCalledExactlyOnceWith(true);
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "true"));
    expect(toggle).toHaveAccessibleDescription("Signing in: finish in Settings → Community.");
  });

  it("checks again from Coming soon", async () => {
    api.getCommunity.mockResolvedValue(community({ stage: "comingSoon", comingSoon: true }));
    api.checkCommunityAgain.mockResolvedValue(signingIn);
    const user = userEvent.setup();
    render(<CommunitySwitch />);
    await user.click(await screen.findByRole("button", { name: "Check again" }));
    expect(api.checkCommunityAgain).toHaveBeenCalledOnce();
    expect(api.setCommunitySwitch).not.toHaveBeenCalled();
    expect(await screen.findByText(/Signing in: finish in Settings/)).toBeInTheDocument();
  });

  it("asks before leaving, and Cancel leaves nothing changed", async () => {
    api.getCommunity.mockResolvedValue(signedIn);
    const user = userEvent.setup();
    render(<CommunitySwitch />);
    const toggle = await theSwitch();
    await user.click(toggle);
    const ask = screen.getByRole("dialog", { name: "Leave Community?" });
    expect(ask).toHaveTextContent(
      "Leaving deletes your profile, your listing, and anything still waiting for you at 8 West, and signs out every computer of yours. What is on this PC stays until you delete it.",
    );
    expect(api.setCommunitySwitch).not.toHaveBeenCalled();
    await user.click(within(ask).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.setCommunitySwitch).not.toHaveBeenCalled();
    expect(api.cancelCommunitySignIn).not.toHaveBeenCalled();
    expect(toggle).toHaveAttribute("aria-checked", "true");
  });

  it("leaves Community only after you say Leave Community", async () => {
    api.getCommunity.mockResolvedValue(signedIn);
    api.setCommunitySwitch.mockResolvedValue(community());
    const user = userEvent.setup();
    render(<CommunitySwitch />);
    const toggle = await theSwitch();
    await user.click(toggle);
    const ask = screen.getByRole("dialog", { name: "Leave Community?" });
    await user.click(within(ask).getByRole("button", { name: "Leave Community" }));
    expect(api.setCommunitySwitch).toHaveBeenCalledExactlyOnceWith(false);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "false"));
  });

  it("says why leaving did not work, and keeps asking", async () => {
    api.getCommunity.mockResolvedValue(signedIn);
    api.setCommunitySwitch.mockRejectedValue({
      kind: "invalidInput",
      message: "Community can't be reached right now. Nothing was changed.",
    });
    const user = userEvent.setup();
    render(<CommunitySwitch />);
    await user.click(await theSwitch());
    const ask = screen.getByRole("dialog", { name: "Leave Community?" });
    await user.click(within(ask).getByRole("button", { name: "Leave Community" }));
    expect(await within(ask).findByRole("alert")).toHaveTextContent(
      "Community can't be reached right now. Nothing was changed.",
    );
  });

  it("only cancels when you turn it off while signing in", async () => {
    api.getCommunity.mockResolvedValue(signingIn);
    api.cancelCommunitySignIn.mockResolvedValue(community());
    const user = userEvent.setup();
    render(<CommunitySwitch />);
    const toggle = await theSwitch();
    expect(toggle).toHaveAttribute("aria-checked", "true");
    await user.click(toggle);
    expect(api.cancelCommunitySignIn).toHaveBeenCalledOnce();
    expect(api.setCommunitySwitch).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "false"));
  });

  it("signs out without asking when you turn it off before you joined", async () => {
    api.getCommunity.mockResolvedValue(joining);
    api.setCommunitySwitch.mockResolvedValue(community());
    const user = userEvent.setup();
    render(<CommunitySwitch />);
    await user.click(await theSwitch());
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.setCommunitySwitch).toHaveBeenCalledExactlyOnceWith(false);
  });

  it("shows an error when turning on does not work", async () => {
    api.getCommunity.mockResolvedValue(community());
    api.setCommunitySwitch.mockRejectedValue({
      kind: "invalidInput",
      message: "Wait a moment: Community is still busy.",
    });
    const user = userEvent.setup();
    render(<CommunitySwitch />);
    await user.click(await theSwitch());
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Wait a moment: Community is still busy.",
    );
  });

  it("says a problem Community remembers once, and not twice", async () => {
    api.getCommunity.mockResolvedValue(
      community({ problem: "The code ran out. Press Community to try again." }),
    );
    render(<CommunitySwitch />);
    expect(await screen.findAllByRole("alert")).toHaveLength(1);
    expect(screen.getByRole("alert")).toHaveTextContent("The code ran out.");
  });

  it("says when Community could not be read", async () => {
    api.getCommunity.mockRejectedValue({ kind: "internal", message: "It broke." });
    render(<CommunitySwitch />);
    expect(await screen.findByText("It broke.")).toBeInTheDocument();
  });
});

describe("Settings → Community", () => {
  it("off: says so, and turns on only when you press the button", async () => {
    api.getCommunity.mockResolvedValue(community());
    api.setCommunitySwitch.mockResolvedValue(signingIn);
    const user = userEvent.setup();
    const { container } = inPage(<CommunitySettings go={go} />);
    expect(await screen.findByText("Community is off.")).toBeInTheDocument();
    expect(api.setCommunitySwitch).not.toHaveBeenCalled();
    expect(a11yProblems(container)).toEqual([]);
    await user.click(screen.getByRole("button", { name: "Turn on Community" }));
    expect(api.setCommunitySwitch).toHaveBeenCalledExactlyOnceWith(true);
    expect(await screen.findByText("Sign in to your 8 West account")).toBeInTheDocument();
  });

  it("coming soon: says nothing was sent, and checks again", async () => {
    api.getCommunity.mockResolvedValue(community({ stage: "comingSoon", comingSoon: true }));
    api.checkCommunityAgain.mockResolvedValue(community({ stage: "comingSoon", comingSoon: true }));
    const user = userEvent.setup();
    const { container } = inPage(<CommunitySettings go={go} />);
    const note = await screen.findByText(/Coming soon\./);
    expect(note.parentElement).toHaveTextContent(
      "Coming soon. 8 West hasn't opened Community yet. Nothing about you was sent.",
    );
    expect(a11yProblems(container)).toEqual([]);
    await user.click(screen.getByRole("button", { name: "Check again" }));
    expect(api.checkCommunityAgain).toHaveBeenCalledOnce();
  });

  it("update needed: says this version can't use it, and goes to Updates", async () => {
    api.getCommunity.mockResolvedValue(community({ stage: "updateNeeded" }));
    const user = userEvent.setup();
    inPage(<CommunitySettings go={go} />);
    const note = await screen.findByText("Update Plenipo to use Community.");
    expect(note.parentElement).toHaveTextContent(
      "Update Plenipo to use Community. This version can't use it.",
    );
    await user.click(screen.getByRole("button", { name: "Go to Updates" }));
    expect(go).toHaveBeenCalledWith({ view: "settings", id: "updates" });
  });

  it("unreachable: shows the problem in a notice, once, and checks again", async () => {
    api.getCommunity.mockResolvedValue(
      community({ stage: "unreachable", problem: "8 West is busy. Nothing was changed." }),
    );
    api.checkCommunityAgain.mockResolvedValue(community({ stage: "comingSoon" }));
    const user = userEvent.setup();
    inPage(<CommunitySettings go={go} />);
    expect(await screen.findAllByText("8 West is busy. Nothing was changed.")).toHaveLength(1);
    expect(screen.queryByRole("alert")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Check again" }));
    expect(api.checkCommunityAgain).toHaveBeenCalledOnce();
  });

  it("unreachable without a reason: says Community can't be reached", async () => {
    api.getCommunity.mockResolvedValue(community({ stage: "unreachable" }));
    inPage(<CommunitySettings go={go} />);
    expect(
      await screen.findByText("Community can't be reached right now. Nothing was changed."),
    ).toBeInTheDocument();
  });

  it("signing in: shows the code, opens the sign-in page, and can be cancelled", async () => {
    api.getCommunity.mockResolvedValue(signingIn);
    api.openCommunityPage.mockResolvedValue(undefined);
    api.cancelCommunitySignIn.mockResolvedValue(community());
    const user = userEvent.setup();
    const { container } = inPage(<CommunitySettings go={go} />);
    expect(
      await screen.findByRole("heading", { name: "Sign in to your 8 West account" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Enter this code:")).toBeInTheDocument();
    const code = screen.getByText("4KQ-7TD");
    expect(code.tagName).toBe("CODE");
    expect(code).toHaveClass("community-code");
    expect(
      screen.getByText(
        "on the account site, and press Allow. Plenipo never sees your password. The code works once, for 10 minutes.",
      ),
    ).toBeInTheDocument();
    expect(a11yProblems(container)).toEqual([]);
    await user.click(screen.getByRole("button", { name: "Open the sign-in page" }));
    expect(api.openCommunityPage).toHaveBeenCalledExactlyOnceWith("signIn");
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(api.cancelCommunitySignIn).toHaveBeenCalledOnce();
    expect(await screen.findByText("Community is off.")).toBeInTheDocument();
  });

  it("says when the sign-in page could not be opened", async () => {
    api.getCommunity.mockResolvedValue(signingIn);
    api.openCommunityPage.mockRejectedValue({
      kind: "invalidInput",
      message: "Plenipo couldn't open your web browser: none is set up.",
    });
    const user = userEvent.setup();
    inPage(<CommunitySettings go={go} />);
    await user.click(await screen.findByRole("button", { name: "Open the sign-in page" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Plenipo couldn't open your web browser",
    );
  });

  describe("joining", () => {
    async function openForm() {
      api.getCommunity.mockResolvedValue(joining);
      const user = userEvent.setup();
      const view = inPage(<CommunitySettings go={go} />);
      const join = await screen.findByRole("button", { name: "Join Community" });
      return { user, join, ...view };
    }

    it("shows who is signed in, and the words of the form", async () => {
      const { container } = await openForm();
      expect(screen.getByText("Frank Gonzalez").tagName).toBe("STRONG");
      expect(screen.getByText("Frank Gonzalez").parentElement).toHaveTextContent(
        "Signed in as Frank Gonzalez.",
      );
      const name = screen.getByRole("textbox", { name: "Your name in Community" });
      expect(name).toHaveAttribute("maxLength", "30");
      expect(name).toHaveAccessibleDescription(
        "3 to 30 letters, numbers, or dashes. People can find you by it.",
      );
      expect(screen.getByText("@")).toBeVisible();
      const months = within(screen.getByRole("combobox", { name: "Birth month" }));
      expect(months.getAllByRole("option").map((o) => o.textContent)).toEqual([
        "Month",
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
      ]);
      const years = within(screen.getByRole("combobox", { name: "Birth year" })).getAllByRole(
        "option",
      );
      expect(years[0]).toHaveTextContent("Year");
      expect(years[1]).toHaveTextContent(String(new Date().getFullYear()));
      expect(years.at(-1)).toHaveTextContent("1900");
      expect(
        screen.getByText("Asked once, and never the day. Community is for people 13 and older."),
      ).toBeInTheDocument();
      expect(screen.getByRole("note")).toHaveTextContent(
        "You'll be listed in the Community directory, so people can find you. Choose Appear offline at any time to not be shown.",
      );
      expect(screen.getByRole("button", { name: "Read the Community terms" })).toBeInTheDocument();
      expect(
        screen.getByRole("checkbox", { name: "I agree to the Community terms" }),
      ).toBeVisible();
      expect(a11yProblems(container)).toEqual([]);
    });

    it("keeps only small letters, numbers, and dashes in the name", async () => {
      const { user } = await openForm();
      const name = screen.getByRole("textbox", { name: "Your name in Community" });
      await user.type(name, "Pat Lee_9-X!@é");
      expect(name).toHaveValue("patlee9-x");
    });

    it("stays off until the name, month, year, and the box are all set", async () => {
      const { user, join } = await openForm();
      const name = screen.getByRole("textbox", { name: "Your name in Community" });
      const agree = screen.getByRole("checkbox", { name: "I agree to the Community terms" });
      expect(join).toBeDisabled();
      await user.type(name, "pat-lee");
      expect(join).toBeDisabled();
      await user.selectOptions(screen.getByRole("combobox", { name: "Birth month" }), "April");
      expect(join).toBeDisabled();
      await user.selectOptions(screen.getByRole("combobox", { name: "Birth year" }), "1990");
      expect(join).toBeDisabled();
      await user.click(agree);
      expect(join).toBeEnabled();
      await user.click(agree);
      expect(join).toBeDisabled();
      await user.click(agree);
      await user.clear(name);
      expect(join).toBeDisabled();
      await user.type(name, "pa");
      expect(join).toBeDisabled();
      await user.type(name, "t");
      expect(join).toBeEnabled();
      expect(api.joinCommunity).not.toHaveBeenCalled();
    });

    it("stays off when 8 West gave no terms to agree to", async () => {
      api.getCommunity.mockResolvedValue({ ...joining, terms: null });
      const user = userEvent.setup();
      inPage(<CommunitySettings go={go} />);
      const join = await screen.findByRole("button", { name: "Join Community" });
      await fillIn(user, "pat-lee", "1990");
      expect(join).toBeDisabled();
    });

    it("opens the terms in your own browser", async () => {
      const { user } = await openForm();
      api.openCommunityPage.mockResolvedValue(undefined);
      await user.click(screen.getByRole("button", { name: "Read the Community terms" }));
      expect(api.openCommunityPage).toHaveBeenCalledExactlyOnceWith("terms");
    });

    it("joins with the name, the month and year, and the terms version", async () => {
      const { user, join } = await openForm();
      api.joinCommunity.mockResolvedValue(signedIn);
      await fillIn(user, "Pat-Lee", "1990");
      await user.click(join);
      expect(api.joinCommunity).toHaveBeenCalledExactlyOnceWith("pat-lee", 4, 1990, "2026-10-01");
      expect(await screen.findByText("@pat-lee")).toBeInTheDocument();
    });

    it("shows what went wrong in an alert, and keeps the form", async () => {
      const { user, join } = await openForm();
      api.joinCommunity.mockRejectedValue({
        kind: "invalidInput",
        message: "Community is for people 13 and older.",
      });
      await fillIn(user, "pat-lee", "2020");
      await user.click(join);
      const alert = await screen.findByRole("alert");
      expect(alert).toHaveTextContent("Community is for people 13 and older.");
      expect(alert).toHaveClass("form-error");
      expect(screen.getByRole("textbox", { name: "Your name in Community" })).toHaveValue(
        "pat-lee",
      );
    });

    it("says it once when Community remembers the same problem", async () => {
      const { user, join } = await openForm();
      const tooYoung = "Community is for people 13 and older.";
      api.joinCommunity.mockImplementation(() => {
        // The app signs this computer out again, and tells every window.
        api.getCommunity.mockResolvedValue(community({ problem: tooYoung }));
        changed.current();
        return Promise.reject(new commands.PlenipoCommandError("invalidInput", tooYoung));
      });
      await fillIn(user, "pat-lee", "2020");
      await user.click(join);
      expect(await screen.findByText("Community is off.")).toBeInTheDocument();
      expect(screen.getAllByRole("alert")).toHaveLength(1);
      expect(screen.getByRole("alert")).toHaveTextContent(tooYoung);
    });
  });

  it("signed in: shows who you are, and leaves only after asking", async () => {
    api.getCommunity.mockResolvedValue(signedIn);
    api.setCommunitySwitch.mockResolvedValue(community());
    const user = userEvent.setup();
    const { container } = inPage(<CommunitySettings go={go} />);
    expect(await screen.findByText("Frank Gonzalez")).toBeInTheDocument();
    expect(screen.getByText("Frank Gonzalez").parentElement).toHaveTextContent(
      "Signed in as Frank Gonzalez.",
    );
    expect(screen.getByText("@pat-lee").parentElement).toHaveTextContent(
      "Your name in Community: @pat-lee",
    );
    expect(screen.queryByText(/8 West paused/)).toBeNull();
    expect(screen.queryByText(/8 West ended/)).toBeNull();
    expect(a11yProblems(container)).toEqual([]);

    await user.click(screen.getByRole("button", { name: "Leave Community" }));
    const ask = screen.getByRole("dialog", { name: "Leave Community?" });
    expect(api.setCommunitySwitch).not.toHaveBeenCalled();
    await user.click(within(ask).getByRole("button", { name: "Cancel" }));
    expect(api.setCommunitySwitch).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Leave Community" }));
    await user.click(
      within(screen.getByRole("dialog", { name: "Leave Community?" })).getByRole("button", {
        name: "Leave Community",
      }),
    );
    expect(api.setCommunitySwitch).toHaveBeenCalledExactlyOnceWith(false);
    expect(await screen.findByText("Community is off.")).toBeInTheDocument();
  });

  it("signed in: signs out of the account, and then offers Sign in", async () => {
    api.getCommunity.mockResolvedValue(signedIn);
    api.signOutOfCommunity.mockResolvedValue(community({ stage: "signedOut", switchedOn: true }));
    api.checkCommunityAgain.mockResolvedValue(signingIn);
    const user = userEvent.setup();
    inPage(<CommunitySettings go={go} />);
    await user.click(await screen.findByRole("button", { name: "Sign out of your account" }));
    expect(api.signOutOfCommunity).toHaveBeenCalledOnce();
    expect(
      await screen.findByText("This PC is signed out of your 8 West account."),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Sign in" }));
    expect(api.checkCommunityAgain).toHaveBeenCalledOnce();
    expect(await screen.findByText("4KQ-7TD")).toBeInTheDocument();
  });

  it("signed out: uses this computer's own words", async () => {
    setSystemWords(SYSTEM_WORDS.linux);
    api.getCommunity.mockResolvedValue(community({ stage: "signedOut", switchedOn: true }));
    inPage(<CommunitySettings go={go} />);
    expect(
      await screen.findByText("This computer is signed out of your 8 West account."),
    ).toBeInTheDocument();
  });

  it("signed in and paused: says until when", async () => {
    const until = Math.floor(Date.UTC(2026, 10, 15, 12) / 1000);
    api.getCommunity.mockResolvedValue({
      ...signedIn,
      member: { ...member, standing: "paused", pausedUntil: BigInt(until) },
    });
    inPage(<CommunitySettings go={go} />);
    const paused = await screen.findByText(/8 West paused your Community/);
    const date = new Date(until * 1000).toLocaleDateString([], {
      year: "numeric",
      month: "long",
      day: "numeric",
    });
    expect(paused).toHaveTextContent(`8 West paused your Community until ${date}.`);
  });

  it("signed in and ended: says 8 West ended it", async () => {
    api.getCommunity.mockResolvedValue({
      ...signedIn,
      member: { ...member, standing: "ended" },
    });
    inPage(<CommunitySettings go={go} />);
    expect(await screen.findByText("8 West ended your Community.")).toBeInTheDocument();
  });

  it("closed: says it is closed for now, and that everything is kept", async () => {
    api.getCommunity.mockResolvedValue(
      community({ stage: "closed", switchedOn: true, accountName: "Frank Gonzalez" }),
    );
    inPage(<CommunitySettings go={go} />);
    const note = await screen.findByText("Community is closed for now.");
    expect(note.parentElement).toHaveTextContent(
      "Community is closed for now. 8 West closed it for a while. Everything on this PC is kept, and Plenipo checks again every hour.",
    );
  });

  it("shows a problem Community remembers, in an alert, in every stage", async () => {
    api.getCommunity.mockResolvedValue(
      community({
        stage: "signedOut",
        switchedOn: true,
        problem: "Signed out here. 8 West couldn't be reached to end this sign-in.",
      }),
    );
    inPage(<CommunitySettings go={go} />);
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveClass("form-error");
    expect(alert).toHaveTextContent("Signed out here.");
  });

  it("reads Community again whenever it changes", async () => {
    api.getCommunity.mockResolvedValue(community());
    inPage(<CommunitySettings go={go} />);
    expect(await screen.findByText("Community is off.")).toBeInTheDocument();
    api.getCommunity.mockResolvedValue(signedIn);
    act(() => changed.current());
    expect(await screen.findByText("@pat-lee")).toBeInTheDocument();
    expect(screen.queryByText("Community is off.")).toBeNull();
  });

  it("says when Community could not be read, and tries again", async () => {
    api.getCommunity.mockRejectedValueOnce({ kind: "internal", message: "It broke." });
    api.getCommunity.mockResolvedValue(community());
    const user = userEvent.setup();
    inPage(<CommunitySettings go={go} />);
    expect(await screen.findByText("It broke.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Community is off.")).toBeInTheDocument();
  });
});

describe("other people's words", () => {
  const hidden = new RegExp("[\\u202E\\u200B]", "u");

  it("never shows a hidden character that could disguise a name", async () => {
    const sneaky = "Frank\u{202E}evil\u{200B}";
    api.getCommunity.mockResolvedValue({ ...joining, accountName: sneaky });
    const first = inPage(<CommunitySettings go={go} />);
    await screen.findByRole("button", { name: "Join Community" });
    expect(document.body.textContent).not.toMatch(hidden);
    expect(document.body.textContent).toContain("Frank\uFFFDevil\uFFFD");
    first.unmount();

    api.getCommunity.mockResolvedValue({ ...signedIn, accountName: sneaky });
    const second = inPage(<CommunitySettings go={go} />);
    await screen.findByText("@pat-lee");
    expect(document.body.textContent).not.toMatch(hidden);
    second.unmount();

    render(<CommunitySwitch />);
    expect(await theSwitch()).toHaveAccessibleDescription(
      "On. Signed in as Frank\uFFFDevil\uFFFD.",
    );
    expect(document.body.textContent).not.toMatch(hidden);
  });

  it("shows a name that looks like a web page as plain text", async () => {
    const markup = '<img src="x" onerror="alert(1)"><b>Frank</b>';
    api.getCommunity.mockResolvedValue({ ...signedIn, accountName: markup });
    const { container } = inPage(<CommunitySettings go={go} />);
    await screen.findByText("@pat-lee");
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("b")).toBeNull();
    expect(container).toHaveTextContent(markup);
  });
});

describe("Settings → Community, in the list", () => {
  it("is a section of Settings, right after Devices", () => {
    const labels = SETTINGS_SECTIONS.map((s) => s.label);
    expect(labels).toContain("Community");
    expect(labels.indexOf("Community")).toBe(labels.indexOf("Devices") + 1);
    const section = SETTINGS_SECTIONS.find((s) => s.id === "community");
    expect(section?.lead).toBe(
      "Your 8 West account in Plenipo: sign in, your Community name, and leaving. Nothing is sent until you turn Community on.",
    );
  });
});
