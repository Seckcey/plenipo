import type { CommunityView } from "@plenipo/types";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { a11yProblems } from "../test/a11y";
import {
  communityView,
  gettingStarted,
  leaderboardView,
  leaderView,
  pointsView,
} from "../test/communityFixtures";
import { CommunitySettings } from "./CommunitySettings";
import { messageTime } from "./messageWords";
import { BADGE_REASONS, BADGE_WORDS } from "./peopleWords";
import { PeoplePage } from "./PeoplePage";
import { forgetPictures } from "./pictures";
import { EARN_ROWS, POINT_REASONS, changeWords, ordinal } from "./rewardsWords";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getCommunity: vi.fn(),
    findInCommunity: vi.fn(),
    communityPicture: vi.fn(),
    communityConversations: vi.fn(),
    communityConversation: vi.fn(),
    communityPoints: vi.fn(),
    communityLeaderboard: vi.fn(),
    communityGettingStarted: vi.fn(),
    closeCommunityGettingStarted: vi.fn(),
    communityBlocked: vi.fn(),
  };
});
// The handler the page gave to listen for messages: calling it is "messages changed".
const heard: { current: () => void } = { current: () => undefined };
vi.mock("../api/events", () => ({
  subscribeCommunity: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeCommunityMessages: vi.fn((handler: () => void) => {
    heard.current = handler;
    return Promise.resolve(() => undefined);
  }),
}));

const api = vi.mocked(commands);
const go = vi.fn();

/** A tiny real PNG (1 × 1), as base64. */
const PNG =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGBgAAAABQABpfZFQAAAAABJRU5ErkJggg==";

/** A member ID in the contract's form (`cm_` and 26 characters). */
const mid = (n: number) => `cm_${String(n).padStart(26, "0")}`;

const member = {
  name: "frank-g",
  adult: true,
  canStart: true,
  standing: "ok",
  pausedUntil: null,
  appearOffline: false,
  hiddenParts: [] as string[],
};
const signedIn = communityView({
  stage: "signedIn",
  switchedOn: true,
  accountName: "Frank Gonzalez",
  member,
  pro: true,
});
const under18: CommunityView = { ...signedIn, member: { ...member, adult: false } };

/** Every character a person cannot see (or that can disguise words), as a row must not show. */
// eslint-disable-next-line no-control-regex
const HIDDEN = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f\u200b-\u200f\u2028-\u202e\u2060-\u2069]/;

const FREE_MONTH = /free months?|month of Pro/i;

beforeEach(() => {
  vi.clearAllMocks();
  // What one test said a command answers is not what the next one hears.
  for (const command of [
    api.getCommunity,
    api.findInCommunity,
    api.communityPicture,
    api.communityConversation,
    api.communityPoints,
    api.communityLeaderboard,
    api.communityGettingStarted,
    api.closeCommunityGettingStarted,
    api.communityBlocked,
  ]) {
    command.mockReset();
  }
  forgetPictures();
  heard.current = () => undefined;
  api.getCommunity.mockResolvedValue(signedIn);
  api.communityConversations.mockResolvedValue([]);
  api.communityConversation.mockResolvedValue(null);
  api.communityGettingStarted.mockResolvedValue(gettingStarted());
  api.communityPoints.mockResolvedValue(pointsView());
  api.communityLeaderboard.mockResolvedValue(leaderboardView([leaderView()]));
  api.communityBlocked.mockResolvedValue([]);
});

afterEach(() => {
  vi.useRealTimers();
});

/** The Community section, once Community has told it you are signed in. */
async function inPage(view: CommunityView = signedIn) {
  api.getCommunity.mockResolvedValue(view);
  // With a clock the test holds, the pretend user's waits move that clock.
  const user = vi.isFakeTimers()
    ? userEvent.setup({ advanceTimers: (ms) => void vi.advanceTimersByTime(ms) })
    : userEvent.setup();
  const shown = render(<PeoplePage go={go} />);
  await screen.findByRole("heading", { name: "Find someone" });
  return { user, ...shown };
}

/** The Community section, on its Leaderboard tab. */
async function inBoard(view: CommunityView = signedIn) {
  const shown = await inPage(view);
  await shown.user.click(screen.getByRole("tab", { name: "Leaderboard" }));
  return shown;
}

const startedBox = () => screen.queryByRole("region", { name: "Getting started" });
const nameBox = () => screen.getByRole("textbox", { name: "Their name in Community" });
const weekList = () => screen.findByRole("list", { name: "Leaderboard: This week" });
const placesOf = (list: HTMLElement) => [...list.children] as HTMLElement[];

describe("Getting started", () => {
  const box = () => screen.findByRole("region", { name: "Getting started" });

  it("ticks the steps that are done, and offers a button for each that is not", async () => {
    api.communityGettingStarted.mockResolvedValue(gettingStarted({ profile: true }));
    const { container } = await inPage();
    const steps = within(await box()).getAllByRole("listitem");
    // Only the three steps this part has.
    expect(steps).toHaveLength(3);
    // Done: a check mark, and the word Done for a screen reader; no button.
    expect(steps[0]?.querySelector(".getting-started__mark svg")).not.toBeNull();
    expect(within(steps[0] as HTMLElement).getByText("Done")).toHaveClass("visually-hidden");
    expect(within(steps[0] as HTMLElement).getByText("Fill in your profile")).toBeInTheDocument();
    expect(within(steps[0] as HTMLElement).queryByRole("button")).not.toBeInTheDocument();
    // Not done: a button, and no check mark.
    for (const [at, name] of [
      [1, "Find someone"],
      [2, "Send a message"],
    ] as const) {
      const step = steps[at] as HTMLElement;
      expect(within(step).getByRole("button", { name })).toBeInTheDocument();
      expect(within(step).queryByText("Done")).not.toBeInTheDocument();
      expect(step.querySelector("svg")).toBeNull();
    }
    expect(a11yProblems(container)).toEqual([]);
  });

  it("ticks every step once each is done, and is gone when all three are", async () => {
    api.communityGettingStarted
      .mockResolvedValueOnce(gettingStarted({ profile: true, foundSomeone: true }))
      .mockResolvedValue(gettingStarted({ profile: true, foundSomeone: true, sentAMessage: true }));
    await inPage();
    const steps = within(await box()).getAllByRole("listitem");
    expect(within(steps[0] as HTMLElement).getByText("Done")).toBeInTheDocument();
    expect(within(steps[1] as HTMLElement).getByText("Done")).toBeInTheDocument();
    expect(within(steps[2] as HTMLElement).queryByText("Done")).not.toBeInTheDocument();
    // A message was sent: Getting started reads again, and there is nothing left to do.
    act(() => heard.current());
    await waitFor(() => expect(startedBox()).not.toBeInTheDocument());
  });

  it("is not there when it was closed, when all three are done, or when it can't be read", async () => {
    api.communityGettingStarted.mockResolvedValue(gettingStarted({ closed: true }));
    const closed = await inPage();
    await waitFor(() => expect(api.communityGettingStarted).toHaveBeenCalledTimes(1));
    await act(() => Promise.resolve());
    expect(startedBox()).not.toBeInTheDocument();
    closed.unmount();

    api.communityGettingStarted.mockResolvedValue(
      gettingStarted({ profile: true, foundSomeone: true, sentAMessage: true }),
    );
    const done = await inPage();
    await waitFor(() => expect(api.communityGettingStarted).toHaveBeenCalledTimes(2));
    await act(() => Promise.resolve());
    expect(startedBox()).not.toBeInTheDocument();
    done.unmount();

    api.communityGettingStarted.mockRejectedValue(
      new commands.PlenipoCommandError("internal", "No."),
    );
    await inPage();
    await waitFor(() => expect(api.communityGettingStarted).toHaveBeenCalledTimes(3));
    await act(() => Promise.resolve());
    // Nothing is said: it is only a help.
    expect(startedBox()).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("has a close button that closes it for good, and hands the cursor back to the tabs", async () => {
    api.closeCommunityGettingStarted.mockResolvedValue(gettingStarted({ closed: true }));
    const { user } = await inPage();
    await box();
    await user.click(screen.getByRole("button", { name: "Close Getting started" }));
    expect(api.closeCommunityGettingStarted).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(startedBox()).not.toBeInTheDocument());
    expect(screen.getByRole("tab", { name: "People" })).toHaveFocus();
  });

  it("stays, and says why, when it can't be closed", async () => {
    api.closeCommunityGettingStarted.mockRejectedValue(
      new commands.PlenipoCommandError("internal", "Plenipo couldn't save that."),
    );
    const { user } = await inPage();
    await box();
    await user.click(screen.getByRole("button", { name: "Close Getting started" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Plenipo couldn't save that.");
    expect(startedBox()).toBeInTheDocument();
  });

  it("takes you to Settings → Community for your profile", async () => {
    const { user } = await inPage();
    await box();
    await user.click(screen.getByRole("button", { name: "Fill in your profile" }));
    expect(go).toHaveBeenCalledWith({ view: "settings", id: "community" });
  });

  it("puts the cursor in the Find someone box", async () => {
    const { user } = await inPage();
    await box();
    expect(nameBox()).not.toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Find someone" }));
    expect(nameBox()).toHaveFocus();
  });

  it("opens Messages for Send a message", async () => {
    const { user } = await inPage();
    await box();
    await user.click(screen.getByRole("button", { name: "Send a message" }));
    expect(screen.getByRole("tab", { name: /Messages/ })).toHaveAttribute("aria-selected", "true");
  });

  it("reads once when the People tab shows, and again each time it shows, and after a lookup", async () => {
    api.findInCommunity.mockResolvedValue({ kind: "noOne" });
    const { user } = await inPage();
    await box();
    expect(api.communityGettingStarted).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("tab", { name: /Messages/ }));
    expect(api.communityGettingStarted).toHaveBeenCalledTimes(1);
    // A message arriving while Messages is open does not read it: nobody is looking.
    act(() => heard.current());
    expect(api.communityGettingStarted).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("tab", { name: "People" }));
    await waitFor(() => expect(api.communityGettingStarted).toHaveBeenCalledTimes(2));
    await user.type(nameBox(), "nobody{Enter}");
    await waitFor(() => expect(api.communityGettingStarted).toHaveBeenCalledTimes(3));
  });

  it("says nothing about a free month", async () => {
    await inPage();
    await box();
    expect(document.body.innerHTML).not.toMatch(FREE_MONTH);
  });
});

describe("The words of rewards", () => {
  it.each([
    [1, "1st"],
    [2, "2nd"],
    [3, "3rd"],
    [4, "4th"],
    [10, "10th"],
    [11, "11th"],
    [12, "12th"],
    [13, "13th"],
    [21, "21st"],
    [22, "22nd"],
    [23, "23rd"],
    [50, "50th"],
    [101, "101st"],
    [111, "111th"],
    [112, "112th"],
    [1001, "1,001st"],
  ])("says place %i as %s", (place, words) => {
    expect(ordinal(place)).toBe(words);
  });

  it("says the badges' one-line reasons, one for each badge", () => {
    expect([...BADGE_REASONS.keys()]).toEqual([...BADGE_WORDS.keys()]);
    expect([...BADGE_REASONS.values()]).toEqual([
      "Joined Community in its first 90 days",
      "Someone's collaborator for 30 days or more",
      "3 links with other organizations, each 30 days or more",
      "Thanked by 10 or more different people",
      "In Community 6 months with no report upheld",
      "Most points last week",
    ]);
  });

  it("says a change in points with its sign", () => {
    expect(changeWords(10)).toBe("+10 points");
    expect(changeWords(1)).toBe("+1 point");
    expect(changeWords(-5)).toBe("\u22125 points");
    expect(changeWords(0)).toBe("0 points");
  });
});

describe("The Leaderboard tab", () => {
  it("is the third tab, after Messages", async () => {
    await inPage();
    expect(screen.getAllByRole("tab").map((t) => t.textContent)).toEqual([
      "People",
      "Messages",
      "Leaderboard",
    ]);
  });

  it("asks for This week, then All time when that is picked, and never on a timer", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    api.communityLeaderboard
      .mockResolvedValueOnce(leaderboardView([leaderView({ name: "week-one" })]))
      .mockResolvedValueOnce(leaderboardView([leaderView({ name: "all-one" })], { allTime: true }));
    const { user } = await inBoard();
    const week = await weekList();
    expect(within(week).getByText("@week-one")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "This week" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(api.communityLeaderboard.mock.calls).toEqual([[false]]);
    expect(api.communityPoints).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole("button", { name: "All time" }));
    const all = await screen.findByRole("list", { name: "Leaderboard: All time" });
    expect(within(all).getByText("@all-one")).toBeInTheDocument();
    expect(screen.queryByText("@week-one")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "All time" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(api.communityLeaderboard.mock.calls).toEqual([[false], [true]]);
    // Picking a list is not a reason to ask for your points again.
    expect(api.communityPoints).toHaveBeenCalledTimes(1);

    // Two hours go by: nothing more is asked.
    await act(async () => {
      vi.advanceTimersByTime(2 * 60 * 60 * 1000);
      await Promise.resolve();
    });
    expect(api.communityLeaderboard).toHaveBeenCalledTimes(2);
    expect(api.communityPoints).toHaveBeenCalledTimes(1);
  });

  it("asks again each time the tab is shown, and not when only People is open", async () => {
    const { user } = await inBoard();
    await weekList();
    expect(api.communityLeaderboard).toHaveBeenCalledTimes(1);
    expect(api.communityPoints).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("tab", { name: "People" }));
    expect(screen.queryByRole("region", { name: "Leaderboard" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Leaderboard" }));
    await weekList();
    expect(api.communityLeaderboard).toHaveBeenCalledTimes(2);
    expect(api.communityPoints).toHaveBeenCalledTimes(2);
  });

  it("shows a place with its number, names, badges, and points", async () => {
    api.communityLeaderboard.mockResolvedValue(
      leaderboardView([
        leaderView({
          place: 1,
          memberId: mid(1),
          name: "pat-lee",
          displayName: "Pat Lee",
          badges: ["founding_member"],
          points: 1250,
        }),
        leaderView({ place: 2, memberId: mid(2), name: "kim-ode", displayName: null, points: 1 }),
      ]),
    );
    const { container } = await inBoard();
    const [pat, kim] = placesOf(await weekList());
    expect(pat).toHaveTextContent(/^Place 1/);
    expect(within(pat as HTMLElement).getByText("Pat Lee")).toBeInTheDocument();
    expect(within(pat as HTMLElement).getByText("@pat-lee")).toBeInTheDocument();
    expect(within(pat as HTMLElement).getByText("1,250 points")).toBeInTheDocument();
    expect(within(pat as HTMLElement).getByText("Founding member")).toBeInTheDocument();
    // With no name on the card, the Community name stands alone.
    expect(within(kim as HTMLElement).getByText("@kim-ode")).toBeInTheDocument();
    expect(within(kim as HTMLElement).getByText("1 point")).toBeInTheDocument();
    expect(a11yProblems(container)).toEqual([]);
  });

  it("shows at most 50 places", async () => {
    api.communityLeaderboard.mockResolvedValue(
      leaderboardView(
        Array.from({ length: 60 }, (_, i) =>
          leaderView({ place: i + 1, memberId: mid(i + 1), name: `member-${i + 1}` }),
        ),
      ),
    );
    await inBoard();
    expect(placesOf(await weekList())).toHaveLength(50);
  });

  it("shows another person's words as text only: a hidden character is a mark, and HTML stays words", async () => {
    api.communityLeaderboard.mockResolvedValue(
      leaderboardView([
        leaderView({
          memberId: mid(1),
          name: "pat\u202elee",
          displayName:
            "<b>Pat</b>\u0007Lee <img src=x onerror=alert(1)>\n<script>alert(1)</script>",
        }),
      ]),
    );
    await inBoard();
    const [row] = placesOf(await weekList());
    expect(row?.textContent).not.toMatch(HIDDEN);
    expect(row).toHaveTextContent("<b>Pat</b>\ufffdLee <img src=x onerror=alert(1)>\ufffd");
    expect(row).toHaveTextContent("<script>alert(1)</script>");
    expect(row).toHaveTextContent("@pat\ufffdlee");
    // No element came out of any of it.
    expect(row?.querySelector("b, img, script, a, [onerror], [href]")).toBeNull();
    expect(within(row as HTMLElement).queryAllByRole("link")).toHaveLength(0);
  });

  it("shows a picture only when the place has one, and only as a data:image/png address", async () => {
    api.communityPicture.mockImplementation((memberId) =>
      Promise.resolve(memberId === mid(3) ? "https://evil.example/x.png" : PNG),
    );
    api.communityLeaderboard.mockResolvedValue(
      leaderboardView([
        leaderView({
          place: 1,
          memberId: mid(1),
          name: "has-one",
          hasPicture: true,
          pictureVersion: "v1",
        }),
        leaderView({
          place: 2,
          memberId: mid(2),
          name: "has-none",
          hasPicture: false,
          pictureVersion: "v1",
        }),
        leaderView({
          place: 3,
          memberId: mid(3),
          name: "bad-one",
          hasPicture: true,
          pictureVersion: "v1",
        }),
      ]),
    );
    await inBoard();
    const list = await weekList();
    await waitFor(() => expect(api.communityPicture).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(list.querySelectorAll("img")).toHaveLength(1));
    await act(() => Promise.resolve());
    const [img] = [...list.querySelectorAll("img")];
    expect(img?.getAttribute("src")).toBe(`data:image/png;base64,${PNG}`);
    expect(img).toHaveAttribute("alt", "");
    // The one that says it has none is never asked for; the one that sent an address is not shown.
    expect(api.communityPicture).toHaveBeenCalledWith(mid(1));
    expect(api.communityPicture).toHaveBeenCalledWith(mid(3));
    expect(api.communityPicture).not.toHaveBeenCalledWith(mid(2));
    for (const place of placesOf(list)) {
      for (const el of place.querySelectorAll("img")) {
        expect(el.getAttribute("src")?.startsWith("data:image/png;base64,")).toBe(true);
      }
    }
  });

  it("shows badges as words, each with its reason as a title and as words for a screen reader", async () => {
    api.communityLeaderboard.mockResolvedValue(
      leaderboardView([
        leaderView({ badges: ["good_neighbor", "future_badge", "top_helper", "good_neighbor"] }),
      ]),
    );
    await inBoard();
    const [row] = placesOf(await weekList());
    const badges = within(row as HTMLElement).getByRole("list", { name: "Badges" });
    const items = within(badges).getAllByRole("listitem");
    expect(items.map((b) => b.firstElementChild?.textContent)).toEqual([
      "Good neighbor",
      "Top helper this week",
    ]);
    expect(items.map((b) => b.getAttribute("title"))).toEqual([
      "Thanked by 10 or more different people",
      "Most points last week",
    ]);
    // The reason is text, so a screen reader says it (a title alone is not read out reliably).
    expect(items[0]).toHaveTextContent("Good neighbor. Thanked by 10 or more different people");
    expect(items[0]?.lastElementChild).toHaveClass("visually-hidden");
    expect(row).not.toHaveTextContent("future_badge");
  });

  it("has a Message button on each place but yours, which opens Messages with a box for that person", async () => {
    api.communityLeaderboard.mockResolvedValue(
      leaderboardView([
        leaderView({ place: 1, memberId: mid(8), name: "pat-lee", displayName: "Pat Lee" }),
        leaderView({ place: 2, memberId: mid(9), name: "frank-g", displayName: "Frank" }),
      ]),
    );
    const { user } = await inBoard();
    const [pat, frank] = placesOf(await weekList());
    expect(within(frank as HTMLElement).queryByRole("button", { name: "Message" })).toBeNull();
    expect(within(frank as HTMLElement).getByText("You")).toBeInTheDocument();
    expect(within(pat as HTMLElement).queryByText("You")).not.toBeInTheDocument();
    await user.click(within(pat as HTMLElement).getByRole("button", { name: "Message" }));
    expect(screen.getByRole("tab", { name: /Messages/ })).toHaveAttribute("aria-selected", "true");
    expect(api.communityConversation).toHaveBeenCalledWith(mid(8), null);
    expect(await screen.findByRole("heading", { level: 2, name: "@pat-lee" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Message to @pat-lee" })).toBeInTheDocument();
  });

  it("says no one is there when the board is empty", async () => {
    api.communityLeaderboard.mockResolvedValue(leaderboardView());
    await inBoard();
    expect(await screen.findByText("No one is on the leaderboard yet.")).toBeVisible();
  });

  it("says in plain words when it can't be read, and asks again on Try again", async () => {
    api.communityLeaderboard
      .mockRejectedValueOnce(
        new commands.PlenipoCommandError("internal", "Community can't be reached right now."),
      )
      .mockResolvedValueOnce(leaderboardView([leaderView({ name: "back-again" })]));
    const { user } = await inBoard();
    const board = screen.getByRole("region", { name: "Leaderboard" });
    expect(await within(board).findByRole("alert")).toHaveTextContent(
      "Community can't be reached right now.",
    );
    await user.click(within(board).getByRole("button", { name: "Try again" }));
    expect(await within(board).findByText("@back-again")).toBeInTheDocument();
    expect(within(board).queryByRole("alert")).not.toBeInTheDocument();
    expect(api.communityLeaderboard).toHaveBeenCalledTimes(2);
  });
});

describe("Your place", () => {
  it.each([
    [1, "Your place: 1st"],
    [2, "Your place: 2nd"],
    [3, "Your place: 3rd"],
    [12, "Your place: 12th"],
    [23, "Your place: 23rd"],
  ])("says place %i in words", async (myPlace, words) => {
    api.communityLeaderboard.mockResolvedValue(
      leaderboardView([leaderView()], { myPlace, myPoints: 48 }),
    );
    await inBoard();
    expect(await screen.findByText(words)).toBeVisible();
    expect(screen.getByText("48 points")).toBeInTheDocument();
    expect(screen.queryByText(/You aren't on the leaderboard/)).not.toBeInTheDocument();
  });

  it("says you aren't on the leaderboard when there is no place for you", async () => {
    api.communityLeaderboard.mockResolvedValue(
      leaderboardView([leaderView()], { myPlace: null, myPoints: 0 }),
    );
    await inBoard();
    expect(
      await screen.findByText(
        "You aren't on the leaderboard (members under 18 and people who appear offline aren't shown).",
      ),
    ).toBeVisible();
    expect(screen.queryByText(/Your place/)).not.toBeInTheDocument();
  });

  it("is the place on the list that is showing", async () => {
    api.communityLeaderboard
      .mockResolvedValueOnce(leaderboardView([leaderView()], { myPlace: 12 }))
      .mockResolvedValueOnce(leaderboardView([leaderView()], { allTime: true, myPlace: 101 }));
    const { user } = await inBoard();
    expect(await screen.findByText("Your place: 12th")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "All time" }));
    expect(await screen.findByText("Your place: 101st")).toBeVisible();
    expect(screen.queryByText("Your place: 12th")).not.toBeInTheDocument();
  });
});

describe("A member under 18", () => {
  it("is never shown the leaderboard, and is never asked for it; the points are theirs", async () => {
    api.communityPoints.mockResolvedValue(pointsView({ total: 40, week: 10 }));
    await inBoard(under18);
    const note = await screen.findByText(
      "Members under 18 aren't on the leaderboard. Your points are below.",
    );
    expect(note).toBeVisible();
    const points = screen.getByRole("region", { name: "Your points" });
    expect(await within(points).findByText("40 points")).toBeInTheDocument();
    // The sentence says "below": the points come after it.
    expect(note.compareDocumentPosition(points) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Leaderboard" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "All time" })).not.toBeInTheDocument();
    expect(screen.queryByRole("list", { name: /^Leaderboard/ })).not.toBeInTheDocument();
    expect(api.communityLeaderboard).not.toHaveBeenCalled();
    expect(api.communityPoints).toHaveBeenCalledTimes(1);
  });

  it("sees How to earn points too", async () => {
    await inBoard(under18);
    expect(await screen.findByText("How to earn points")).toBeInTheDocument();
    expect(api.communityLeaderboard).not.toHaveBeenCalled();
  });

  it("is the only one told so: an adult is shown the board, and not that sentence", async () => {
    await inBoard(signedIn);
    await weekList();
    expect(api.communityLeaderboard).toHaveBeenCalledTimes(1);
    expect(
      screen.queryByText("Members under 18 aren't on the leaderboard. Your points are below."),
    ).not.toBeInTheDocument();
  });
});

describe("Your points", () => {
  const AT = 1_790_000_000;
  const DAY = 86_400;
  const REASONS: [string, string][] = [
    ["contact_accepted", "Someone accepted your message request"],
    ["thanks", "Someone thanked you"],
    ["invite_joined", "Someone you invited by email joined"],
    ["link_7_days", "A link lasted 7 days"],
    ["collab_7_days", "A collaboration lasted 7 days"],
    ["getting_started", "You finished Getting started"],
    ["good_month", "A month with no report upheld against you"],
    ["invite_bought_pro", "Someone you invited bought Pro"],
    ["taken_back", "Points taken back"],
    ["removed_by_8west", "Removed by 8 West"],
  ];
  const panel = () => screen.getByRole("region", { name: "Your points" });

  it("shows your total, this week, and the thanks", async () => {
    api.communityPoints.mockResolvedValue(pointsView({ total: 1250, week: 35, thankedBy: 12 }));
    await inBoard();
    expect(await within(panel()).findByText("1,250 points")).toBeInTheDocument();
    const totals = within(panel());
    expect(totals.getByText("Total")).toBeInTheDocument();
    expect(totals.getByText("This week")).toBeInTheDocument();
    expect(totals.getByText("35 points")).toBeInTheDocument();
    expect(totals.getByText("Thanked by 12 people")).toBeInTheDocument();
  });

  it("says Thanked by 1 person for one, and nothing for none", async () => {
    api.communityPoints.mockResolvedValue(pointsView({ thankedBy: 1 }));
    const first = await inBoard();
    expect(await within(panel()).findByText("Thanked by 1 person")).toBeInTheDocument();
    first.unmount();
    api.communityPoints.mockResolvedValue(pointsView({ thankedBy: 0, total: 3 }));
    await inBoard();
    expect(await within(panel()).findByText("3 points")).toBeInTheDocument();
    expect(within(panel()).queryByText(/Thanked/)).not.toBeInTheDocument();
  });

  it("shows your badges as words with their reasons, and leaves out a badge it does not know", async () => {
    api.communityPoints.mockResolvedValue(
      pointsView({ badges: ["founding_member", "future_badge", "trusted"] }),
    );
    await inBoard();
    const badges = await within(panel()).findByRole("list", { name: "Your badges" });
    const items = within(badges).getAllByRole("listitem");
    expect(items.map((b) => b.firstElementChild?.textContent)).toEqual([
      "Founding member",
      "Trusted",
    ]);
    expect(items.map((b) => b.getAttribute("title"))).toEqual([
      "Joined Community in its first 90 days",
      "In Community 6 months with no report upheld",
    ]);
    expect(items[1]).toHaveTextContent("Trusted. In Community 6 months with no report upheld");
    expect(panel()).not.toHaveTextContent("future_badge");
  });

  it("says so when you have no badges yet", async () => {
    await inBoard();
    expect(await within(panel()).findByText("No badges yet.")).toBeInTheDocument();
  });

  it("says each change in plain words, with its points and your own time, newest first", async () => {
    const recent = REASONS.map(([reason], i) => ({
      points: reason === "taken_back" ? -5 : reason === "removed_by_8west" ? -10 : i + 1,
      reason,
      at: AT - i * DAY,
    }));
    api.communityPoints.mockResolvedValue(pointsView({ recent }));
    await inBoard();
    const list = await within(panel()).findByRole("list", { name: "Recent changes" });
    const rows = within(list).getAllByRole("listitem");
    expect(rows).toHaveLength(10);
    REASONS.forEach(([reason, words], i) => {
      const row = rows[i] as HTMLElement;
      const change = recent[i];
      expect(within(row).getByText(words), reason).toBeInTheDocument();
      expect(within(row).getByText(changeWords(change?.points ?? 0))).toBeInTheDocument();
      const when = within(row).getByText(messageTime(AT - i * DAY));
      expect(when.tagName).toBe("TIME");
      expect(when).toHaveAttribute("datetime", new Date((AT - i * DAY) * 1000).toISOString());
    });
    expect(within(rows[8] as HTMLElement).getByText("\u22125 points")).toBeInTheDocument();
    expect(within(rows[0] as HTMLElement).getByText("+1 point")).toBeInTheDocument();
    // The contract's words are never shown as they are.
    expect(panel().textContent).not.toMatch(/contact_accepted|removed_by_8west|link_7_days/);
  });

  it("says a reason for every word the contract has, and leaves out one it does not know", async () => {
    expect([...POINT_REASONS.entries()]).toEqual(REASONS);
    api.communityPoints.mockResolvedValue(
      pointsView({
        recent: [
          { points: 7, reason: "future_reason", at: AT },
          { points: 2, reason: "contact_accepted", at: AT },
        ],
      }),
    );
    await inBoard();
    const list = await within(panel()).findByRole("list", { name: "Recent changes" });
    const rows = within(list).getAllByRole("listitem");
    // The unknown one is left out whole: no words, no points.
    expect(rows).toHaveLength(1);
    expect(rows[0]).toHaveTextContent("+2 points");
    expect(panel().textContent).not.toMatch(/future_reason|\+7/);
  });

  it("says so when there are no changes yet", async () => {
    await inBoard();
    expect(await within(panel()).findByText("No changes yet.")).toBeInTheDocument();
  });

  it("says in plain words when your points can't be read, and asks again on Try again", async () => {
    api.communityPoints
      .mockRejectedValueOnce(new commands.PlenipoCommandError("internal", "Try again in an hour."))
      .mockResolvedValueOnce(pointsView({ total: 9 }));
    const { user } = await inBoard();
    expect(await within(panel()).findByRole("alert")).toHaveTextContent("Try again in an hour.");
    await user.click(within(panel()).getByRole("button", { name: "Try again" }));
    expect(await within(panel()).findByText("9 points")).toBeInTheDocument();
    expect(within(panel()).queryByRole("alert")).not.toBeInTheDocument();
    expect(api.communityPoints).toHaveBeenCalledTimes(2);
  });
});

describe("How to earn points", () => {
  const ROWS: [string, string][] = [
    ["Someone accepts your message request (once for each person)", "2"],
    ["Someone thanks you", "3"],
    ["Someone you invited joins Community", "5"],
    ["A link with another organization lasts 7 days", "10"],
    ["You help someone as a collaborator for 7 days, or they help you", "10"],
    ["You finish Getting started (once)", "10"],
    ["Each month in Community with no report against you upheld", "5"],
    ["Someone you invited buys Pro and keeps it past 14 days", "25"],
  ];

  async function opened() {
    const shown = await inBoard();
    const summary = screen.getByText("How to earn points");
    const section = summary.closest("details") as HTMLDetailsElement;
    return { ...shown, summary, section };
  }

  it("is a section that opens, closed to begin with", async () => {
    const { user, summary, section } = await opened();
    expect(summary.tagName).toBe("SUMMARY");
    expect(section.open).toBe(false);
    await user.click(summary);
    expect(section.open).toBe(true);
    await user.click(summary);
    expect(section.open).toBe(false);
  });

  it("lists the eight ways, with their points", async () => {
    const { user, summary, section } = await opened();
    await user.click(summary);
    const table = within(section).getByRole("table", { name: "Ways to earn points" });
    const rows = within(table).getAllByRole("row");
    // The heads, then the eight.
    expect(rows).toHaveLength(9);
    expect(within(rows[0] as HTMLElement).getAllByRole("columnheader")).toHaveLength(2);
    expect(
      rows.slice(1).map((r) =>
        within(r)
          .getAllByRole("cell")
          .map((c) => c.textContent),
      ),
    ).toEqual(ROWS);
    expect(EARN_ROWS).toHaveLength(8);
  });

  it("says what earns nothing, the limits, and that only an email invitation counts", async () => {
    const { user, summary, section } = await opened();
    await user.click(summary);
    expect(
      within(section).getByText(
        "Nothing for messages sent, requests sent, invitations sent, links asked for, or time in Plenipo. At most 100 points a week, and 20 a month from any one person. Points have no money value.",
      ),
    ).toBeVisible();
    expect(within(section).getByText("Only people you invite by email count.")).toBeVisible();
  });
});

describe("A free month", () => {
  it("is not mentioned anywhere on the Community page", async () => {
    api.communityPoints.mockResolvedValue(
      pointsView({
        total: 120,
        badges: ["founding_member"],
        recent: [{ points: 25, reason: "invite_bought_pro", at: 1_790_000_000 }],
      }),
    );
    api.communityLeaderboard.mockResolvedValue(
      leaderboardView([leaderView({ badges: ["helper"] })], { myPlace: 4 }),
    );
    const { user } = await inPage();
    // People (with Getting started), then the Leaderboard with How to earn points open.
    expect(document.body.innerHTML).not.toMatch(FREE_MONTH);
    await user.click(screen.getByRole("tab", { name: "Leaderboard" }));
    await weekList();
    await within(screen.getByRole("region", { name: "Your points" })).findByText("120 points");
    await user.click(screen.getByText("How to earn points"));
    await user.click(screen.getByRole("button", { name: "All time" }));
    await screen.findByRole("list", { name: "Leaderboard: All time" });
    expect(document.body.innerHTML).not.toMatch(FREE_MONTH);
    expect(document.body.textContent).not.toMatch(FREE_MONTH);
  });
});

describe("Settings → Community", () => {
  const inSettings = () => render(<CommunitySettings go={go} />);

  it("shows Thanked by N people and your badges under your name", async () => {
    api.communityPoints.mockResolvedValue(
      pointsView({ thankedBy: 12, badges: ["founding_member", "helper"] }),
    );
    inSettings();
    expect(await screen.findByText("Thanked by 12 people")).toBeInTheDocument();
    const name = screen.getByText(/Your name in Community:/);
    const thanks = screen.getByText("Thanked by 12 people");
    const badges = screen.getByRole("list", { name: "Your badges" });
    expect(
      within(badges)
        .getAllByRole("listitem")
        .map((b) => b.firstElementChild?.textContent),
    ).toEqual(["Founding member", "Helper"]);
    expect(within(badges).getAllByRole("listitem")[1]).toHaveAttribute(
      "title",
      "Someone's collaborator for 30 days or more",
    );
    // Under your name, in that order.
    expect(name.compareDocumentPosition(thanks) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(thanks.compareDocumentPosition(badges) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    // Your points are read once, when it shows.
    expect(api.communityPoints).toHaveBeenCalledTimes(1);
  });

  it("says Thanked by 1 person for one", async () => {
    api.communityPoints.mockResolvedValue(pointsView({ thankedBy: 1 }));
    inSettings();
    expect(await screen.findByText("Thanked by 1 person")).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "Your badges" })).not.toBeInTheDocument();
  });

  it("shows nothing when there is no thanks and no badge yet", async () => {
    inSettings();
    await screen.findByText(/Your name in Community:/);
    await waitFor(() => expect(api.communityPoints).toHaveBeenCalledTimes(1));
    await act(() => Promise.resolve());
    expect(screen.queryByText(/Thanked/)).not.toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "Your badges" })).not.toBeInTheDocument();
  });

  it("shows nothing, and no error, when your points can't be read", async () => {
    api.communityPoints.mockRejectedValue(new commands.PlenipoCommandError("internal", "Down."));
    inSettings();
    await screen.findByText(/Your name in Community:/);
    await waitFor(() => expect(api.communityPoints).toHaveBeenCalledTimes(1));
    await act(() => Promise.resolve());
    expect(screen.queryByText(/Thanked/)).not.toBeInTheDocument();
    expect(screen.queryByText("Down.")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("does not ask for your points before you are signed in with a member", async () => {
    api.getCommunity.mockResolvedValue(communityView({ stage: "off" }));
    inSettings();
    await screen.findByText("Community is off.");
    expect(api.communityPoints).not.toHaveBeenCalled();
  });

  it("says nothing about a free month", async () => {
    api.communityPoints.mockResolvedValue(pointsView({ thankedBy: 3, badges: ["trusted"] }));
    inSettings();
    await screen.findByText("Thanked by 3 people");
    expect(document.body.innerHTML).not.toMatch(FREE_MONTH);
  });
});
