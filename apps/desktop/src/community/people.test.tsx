import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { CardView, CommunityView, Stage } from "@plenipo/types";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { Sidebar } from "../components/Sidebar";
import { a11yProblems } from "../test/a11y";
import { cardView, communityView, peoplePage } from "../test/communityFixtures";
import { CommunityCard } from "./CommunityCard";
import { BADGE_WORDS } from "./peopleWords";
import { INVITE_SENT, NO_ONE, PeoplePage } from "./PeoplePage";
import { forgetPictures } from "./pictures";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getCommunity: vi.fn(),
    communityDirectory: vi.fn(),
    communityNewThisWeek: vi.fn(),
    findInCommunity: vi.fn(),
    communityPicture: vi.fn(),
    inviteToCommunity: vi.fn(),
    shareMyCommunityProfile: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeCommunity: vi.fn(() => Promise.resolve(() => undefined)),
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

/** The Community section, once Community has told it you are signed in. */
async function inPage(view: CommunityView = signedIn) {
  api.getCommunity.mockResolvedValue(view);
  const shown = render(<PeoplePage go={go} />);
  await screen.findByRole("heading", { name: "Find someone" });
  return shown;
}

const part = (name: string) => within(screen.getByRole("region", { name }));
const nameBox = () => screen.getByRole("textbox", { name: "Their name in Community" });
const searchBox = () =>
  screen.getByRole("searchbox", { name: "Search by name, company, or what a business does" });

/** Cards on the page: each is an article named by its Community name. */
const cardsIn = (root: ReturnType<typeof part>) => root.queryAllByRole("article");

/** Every character a person cannot see (or that can disguise words), as the card must not show. */
// eslint-disable-next-line no-control-regex
const HIDDEN = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f\u200b-\u200f\u2028-\u202e\u2060-\u2069]/;

beforeEach(() => {
  vi.clearAllMocks();
  forgetPictures();
});

afterEach(() => {
  // A test may have put its own clipboard on the window.
  Reflect.deleteProperty(navigator, "clipboard");
});

describe("Community on the strip", () => {
  const strip = (props: { communityOn?: boolean } = {}) =>
    render(
      <Sidebar
        current="home"
        onNavigate={vi.fn()}
        activeCount={0}
        workingCount={0}
        {...(props.communityOn === undefined ? {} : { communityOn: props.communityOn })}
      />,
    );

  it("is there while Community's switch is on, with its tooltip", () => {
    strip({ communityOn: true });
    expect(screen.getByRole("button", { name: "Community" })).toHaveAttribute(
      "title",
      "Community: find people who use Plenipo",
    );
  });

  it("is not there while the switch is off, or when nothing says it is on", () => {
    const first = strip({ communityOn: false });
    expect(screen.queryByRole("button", { name: /Community/ })).not.toBeInTheDocument();
    // The other sections are there as ever.
    expect(screen.getByRole("button", { name: "Activity" })).toBeInTheDocument();
    first.unmount();
    strip();
    expect(screen.queryByRole("button", { name: /Community/ })).not.toBeInTheDocument();
  });

  it("is between Activity and Settings", () => {
    strip({ communityOn: true });
    const names = within(screen.getByRole("navigation", { name: "Main" }))
      .getAllByRole("button")
      .map((b) => b.textContent);
    const at = names.indexOf("Community");
    expect(names[at - 1]).toBe("Activity");
    expect(names[at + 1]).toBe("Settings");
  });
});

describe("The Community section", () => {
  it("has one tab, People, and the parts of it", async () => {
    const { container } = await inPage();
    expect(screen.getByRole("heading", { level: 1, name: "Community" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "People" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel", { name: "People" })).toBeInTheDocument();
    for (const title of [
      "Find someone",
      "Directory",
      "New this week",
      "Invite by email",
      "Share my profile",
    ]) {
      expect(screen.getByRole("heading", { level: 2, name: title })).toBeInTheDocument();
    }
    expect(a11yProblems(container)).toEqual([]);
  });

  it("asks 8 West for nothing when it opens", async () => {
    await inPage();
    // One account sees only so many cards a day: a look at the page uses none.
    expect(api.communityDirectory).not.toHaveBeenCalled();
    expect(api.communityNewThisWeek).not.toHaveBeenCalled();
    expect(api.findInCommunity).not.toHaveBeenCalled();
    expect(api.communityPicture).not.toHaveBeenCalled();
  });

  it.each<Stage>([
    "off",
    "comingSoon",
    "updateNeeded",
    "unreachable",
    "signingIn",
    "joining",
    "signedOut",
    "closed",
  ])("says so, with a way to Settings → Community, when Community is %s", async (stage) => {
    api.getCommunity.mockResolvedValue(communityView({ stage, switchedOn: true }));
    const user = userEvent.setup();
    render(<PeoplePage go={go} />);
    expect(await screen.findByRole("note")).toHaveTextContent(
      "You are not signed in to Community on this PC. Open Settings → Community to sign in.",
    );
    expect(screen.queryByRole("heading", { name: "Find someone" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Go to Settings → Community" }));
    expect(go).toHaveBeenCalledWith({ view: "settings", id: "community" });
  });

  it("says when Community can't be read, and tries again", async () => {
    api.getCommunity.mockRejectedValueOnce(new commands.PlenipoCommandError("internal", "down"));
    api.getCommunity.mockResolvedValue(signedIn);
    const user = userEvent.setup();
    render(<PeoplePage go={go} />);
    expect(await screen.findByText("Couldn't load Community")).toBeInTheDocument();
    expect(screen.getByText("down")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("heading", { name: "Find someone" })).toBeInTheDocument();
  });
});

describe("Find someone", () => {
  it("shows a card for a name that is found, and sends the name as it was typed", async () => {
    api.findInCommunity.mockResolvedValue({
      kind: "card",
      card: cardView({ name: "pat-lee", displayName: "Pat Lee", company: "Lee Builders" }),
    });
    const user = userEvent.setup();
    await inPage();
    await user.type(nameBox(), "@Pat-Lee");
    await user.click(screen.getByRole("button", { name: "Find" }));
    // Plenipo (the Rust side) makes the letters small and drops the "@".
    expect(api.findInCommunity).toHaveBeenCalledWith("@Pat-Lee");
    const card = await part("Find someone").findByRole("article", { name: "@pat-lee" });
    expect(within(card).getByText("Pat Lee")).toBeInTheDocument();
    expect(within(card).getByText("Lee Builders")).toBeInTheDocument();
  });

  it("shows the @ before the box, and Find waits for a name", async () => {
    const user = userEvent.setup();
    await inPage();
    expect(screen.getByRole("button", { name: "Find" })).toBeDisabled();
    await user.type(nameBox(), "@");
    expect(screen.getByRole("button", { name: "Find" })).toBeDisabled();
    await user.type(nameBox(), "x");
    expect(screen.getByRole("button", { name: "Find" })).toBeEnabled();
    expect(part("Find someone").getByText("@")).toBeInTheDocument();
  });

  it("says only that a message request can be sent, for a request-only name", async () => {
    api.findInCommunity.mockResolvedValue({
      kind: "requestOnly",
      memberId: mid(7),
      name: "kim-ode",
    });
    const user = userEvent.setup();
    await inPage();
    await user.type(nameBox(), "kim-ode{Enter}");
    expect(await screen.findByText("@kim-ode can only be sent a message request.")).toBeVisible();
    expect(cardsIn(part("Find someone"))).toHaveLength(0);
  });

  it("says only that no one has the name, for no one", async () => {
    api.findInCommunity.mockResolvedValue({ kind: "noOne" });
    const user = userEvent.setup();
    await inPage();
    await user.type(nameBox(), "nobody-here");
    await user.click(screen.getByRole("button", { name: "Find" }));
    expect(await screen.findByText("No one in Community has that name.")).toBeVisible();
    expect(NO_ONE).toBe("No one in Community has that name.");
    expect(cardsIn(part("Find someone"))).toHaveLength(0);
  });

  it("clears the answer when another name is typed", async () => {
    api.findInCommunity.mockResolvedValue({ kind: "noOne" });
    const user = userEvent.setup();
    await inPage();
    await user.type(nameBox(), "nobody");
    await user.click(screen.getByRole("button", { name: "Find" }));
    await screen.findByText(NO_ONE);
    await user.type(nameBox(), "2");
    expect(screen.queryByText(NO_ONE)).not.toBeInTheDocument();
  });

  it("says in plain words when it can't find out", async () => {
    api.findInCommunity.mockRejectedValue(
      new commands.PlenipoCommandError("internal", "Too many look-ups. Try again in an hour."),
    );
    const user = userEvent.setup();
    await inPage();
    await user.type(nameBox(), "pat-lee{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Too many look-ups. Try again in an hour.",
    );
    expect(cardsIn(part("Find someone"))).toHaveLength(0);
  });
});

describe("Directory", () => {
  const first = [
    cardView({ memberId: mid(1), name: "pat-lee", displayName: "Pat Lee" }),
    cardView({ memberId: mid(2), name: "kim-ode", displayName: "Kim Ode" }),
  ];
  const list = () => part("Directory").getByRole("list", { name: "People in the directory" });

  it("searches with the words, the kind, and the place, and shows the cards", async () => {
    api.communityDirectory.mockResolvedValue(peoplePage(first));
    const user = userEvent.setup();
    await inPage();
    await user.type(searchBox(), "  builders ");
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Kind of business" }),
      "Construction",
    );
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Where" }),
      "California (United States)",
    );
    await user.click(screen.getByRole("button", { name: "Search" }));
    expect(api.communityDirectory).toHaveBeenCalledWith("builders", "construction", "US-CA", "");
    await waitFor(() => expect(within(list()).getAllByRole("article")).toHaveLength(2));
    expect(within(list()).getByRole("article", { name: "@kim-ode" })).toBeInTheDocument();
    // 20 at a time: with no next page there is no Show more.
    expect(part("Directory").queryByRole("button", { name: "Show more" })).not.toBeInTheDocument();
  });

  it("offers Any kind of business and Anywhere, and the same lists as your profile", async () => {
    const user = userEvent.setup();
    await inPage();
    const kind = screen.getByRole("combobox", { name: "Kind of business" });
    const where = screen.getByRole("combobox", { name: "Where" });
    expect(kind).toHaveDisplayValue("Any kind of business");
    expect(where).toHaveDisplayValue("Anywhere");
    expect(within(kind).getAllByRole("option")).toHaveLength(31);
    expect(within(where).getByRole("option", { name: "Canada" })).toHaveValue("CA");
    expect(within(where).queryByRole("option", { name: "Not shown" })).not.toBeInTheDocument();
    // A country alone (US), and a state, are both choices.
    api.communityDirectory.mockResolvedValue(peoplePage([]));
    await user.selectOptions(where, "United States");
    await user.click(screen.getByRole("button", { name: "Search" }));
    expect(api.communityDirectory).toHaveBeenCalledWith("", "", "US", "");
  });

  it("limits the search to 60 characters", async () => {
    await inPage();
    expect(searchBox()).toHaveAttribute("maxlength", "60");
  });

  it("shows more with the cursor, adds the cards, and keeps what was searched", async () => {
    api.communityDirectory
      .mockResolvedValueOnce(peoplePage(first, "cursor-2"))
      .mockResolvedValueOnce(
        peoplePage(
          [
            // A card already on screen is not shown twice.
            first[0] as CardView,
            cardView({ memberId: mid(3), name: "sam-ray", displayName: "Sam Ray" }),
          ],
          null,
        ),
      );
    const user = userEvent.setup();
    await inPage();
    await user.type(searchBox(), "builders");
    await user.click(screen.getByRole("button", { name: "Search" }));
    const more = await part("Directory").findByRole("button", { name: "Show more" });
    // Typing since the search changes nothing about the next page.
    await user.type(searchBox(), " and more");
    await user.click(more);
    expect(api.communityDirectory).toHaveBeenLastCalledWith("builders", "", "", "cursor-2");
    await waitFor(() => expect(within(list()).getAllByRole("article")).toHaveLength(3));
    expect(within(list()).getAllByRole("article", { name: "@pat-lee" })).toHaveLength(1);
    expect(part("Directory").queryByRole("button", { name: "Show more" })).not.toBeInTheDocument();
  });

  it("starts again with a new search, never mixing two", async () => {
    api.communityDirectory
      .mockResolvedValueOnce(peoplePage(first, "cursor-2"))
      .mockResolvedValueOnce(
        peoplePage([cardView({ memberId: mid(9), name: "lee-new", displayName: null })]),
      );
    const user = userEvent.setup();
    await inPage();
    await user.click(screen.getByRole("button", { name: "Search" }));
    await part("Directory").findByRole("button", { name: "Show more" });
    await user.type(searchBox(), "new");
    await user.click(screen.getByRole("button", { name: "Search" }));
    expect(api.communityDirectory).toHaveBeenLastCalledWith("new", "", "", "");
    await waitFor(() => expect(within(list()).getAllByRole("article")).toHaveLength(1));
    expect(within(list()).getByRole("article", { name: "@lee-new" })).toBeInTheDocument();
    expect(part("Directory").queryByRole("button", { name: "Show more" })).not.toBeInTheDocument();
  });

  it("says No one matches. when no one does", async () => {
    api.communityDirectory.mockResolvedValue(peoplePage([]));
    const user = userEvent.setup();
    await inPage();
    expect(screen.queryByText("No one matches.")).not.toBeInTheDocument();
    await user.type(searchBox(), "zzz{Enter}");
    expect(await screen.findByText("No one matches.")).toBeVisible();
  });

  it("says in plain words when the search can't be done", async () => {
    api.communityDirectory.mockRejectedValue(
      new commands.PlenipoCommandError("internal", "You have looked at enough people today."),
    );
    const user = userEvent.setup();
    await inPage();
    await user.click(screen.getByRole("button", { name: "Search" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "You have looked at enough people today.",
    );
    expect(screen.queryByText("No one matches.")).not.toBeInTheDocument();
  });
});

describe("New this week", () => {
  it("shows the first 20 on a button, and more with the cursor", async () => {
    api.communityNewThisWeek
      .mockResolvedValueOnce(
        peoplePage([cardView({ memberId: mid(4), name: "new-one", displayName: "New One" })], "n2"),
      )
      .mockResolvedValueOnce(
        peoplePage([cardView({ memberId: mid(5), name: "new-two", displayName: "New Two" })]),
      );
    const user = userEvent.setup();
    await inPage();
    const here = part("New this week");
    expect(cardsIn(here)).toHaveLength(0);
    await user.click(here.getByRole("button", { name: "Show new people" }));
    expect(api.communityNewThisWeek).toHaveBeenLastCalledWith("");
    await waitFor(() => expect(cardsIn(here)).toHaveLength(1));
    expect(here.queryByRole("button", { name: "Show new people" })).not.toBeInTheDocument();
    await user.click(here.getByRole("button", { name: "Show more" }));
    expect(api.communityNewThisWeek).toHaveBeenLastCalledWith("n2");
    await waitFor(() => expect(cardsIn(here)).toHaveLength(2));
    expect(here.queryByRole("button", { name: "Show more" })).not.toBeInTheDocument();
  });

  it("says when no one is new", async () => {
    api.communityNewThisWeek.mockResolvedValueOnce(peoplePage([]));
    const user = userEvent.setup();
    await inPage();
    await user.click(screen.getByRole("button", { name: "Show new people" }));
    expect(await screen.findByText("No one is new this week.")).toBeVisible();
  });

  it("keeps the button to try again after a problem", async () => {
    api.communityNewThisWeek.mockRejectedValueOnce(
      new commands.PlenipoCommandError("internal", "Community can't be reached right now."),
    );
    api.communityNewThisWeek.mockResolvedValueOnce(
      peoplePage([cardView({ memberId: mid(4), name: "new-one" })]),
    );
    const user = userEvent.setup();
    await inPage();
    await user.click(screen.getByRole("button", { name: "Show new people" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Community can't be reached right now.",
    );
    await user.click(screen.getByRole("button", { name: "Show new people" }));
    expect(await part("New this week").findByRole("article")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});

describe("Invite by email", () => {
  const box = () => screen.getByRole("textbox", { name: "Their email address" });

  it("sends the address and says the same words whatever the answer is", async () => {
    api.inviteToCommunity.mockResolvedValue(undefined);
    const user = userEvent.setup();
    await inPage();
    expect(screen.getByRole("button", { name: "Invite" })).toBeDisabled();
    await user.type(box(), " new@example.com ");
    await user.click(screen.getByRole("button", { name: "Invite" }));
    expect(api.inviteToCommunity).toHaveBeenCalledWith("new@example.com");
    const words = await screen.findByText(INVITE_SENT);
    expect(words).toHaveTextContent(
      "If that address can join Community, 8 West will email it an invitation.",
    );
    expect(box()).toHaveValue("");
    // Another address, the same words: nothing says if it has an account.
    await user.type(box(), "someone.else@example.org{Enter}");
    expect(api.inviteToCommunity).toHaveBeenLastCalledWith("someone.else@example.org");
    expect((await screen.findByText(INVITE_SENT)).textContent).toBe(words.textContent);
  });

  it("shows the plain words of a problem, and not the sentence for a success", async () => {
    api.inviteToCommunity.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "Inviting by email is part of Pro."),
    );
    const user = userEvent.setup();
    await inPage();
    await user.type(box(), "new@example.com");
    await user.click(screen.getByRole("button", { name: "Invite" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Inviting by email is part of Pro.");
    expect(screen.queryByText(INVITE_SENT)).not.toBeInTheDocument();
    // The address stays, to be fixed or tried again.
    expect(box()).toHaveValue("new@example.com");
    await user.type(box(), "m");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});

describe("Share my profile", () => {
  const link = "https://getplenipo.com/c/frank-g";
  /** 21 × 21 squares: every third one dark. */
  const cells = Array.from({ length: 21 * 21 }, (_, i) => (i % 3 === 0 ? "1" : "0")).join("");
  const dark = cells.split("").filter((c) => c === "1").length;
  const shared = { link, qrSize: 21, qrCells: cells };

  /** Put a clipboard on the window (after `userEvent.setup()`, which brings its own). */
  function clipboard(writeText: ReturnType<typeof vi.fn>) {
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
  }

  it("shows the link as text, and draws the picture code, with what the page says", async () => {
    api.shareMyCommunityProfile.mockResolvedValue(shared);
    const user = userEvent.setup();
    const { container } = await inPage();
    const here = part("Share my profile");
    expect(
      here.getByText(
        "Anyone can open this page. It says how to find you in Plenipo, and nothing else about you.",
      ),
    ).toBeInTheDocument();
    expect(here.queryByText(link)).not.toBeInTheDocument();
    await user.click(here.getByRole("button", { name: "Share my profile" }));
    const text = await here.findByText(link);
    // It is words to select and copy: never a link that opens by itself.
    expect(text.tagName).toBe("CODE");
    expect(here.queryByRole("link")).not.toBeInTheDocument();
    const code = here.getByRole("img", { name: "Picture code for your profile link" });
    const squares = code.querySelector(".picture-code__squares")?.getAttribute("d") ?? "";
    expect(squares.split("M").length - 1).toBe(dark);
    expect(a11yProblems(container)).toEqual([]);
  });

  it("copies the link, and says it did", async () => {
    api.shareMyCommunityProfile.mockResolvedValue(shared);
    const user = userEvent.setup();
    const writeText = vi.fn().mockResolvedValue(undefined);
    await inPage();
    clipboard(writeText);
    await user.click(screen.getByRole("button", { name: "Share my profile" }));
    await user.click(await screen.findByRole("button", { name: "Copy link" }));
    expect(writeText).toHaveBeenCalledWith(link);
    expect(await screen.findByText("Copied.")).toBeVisible();
  });

  it("says so when the link can't be copied, and the link stays to copy by hand", async () => {
    api.shareMyCommunityProfile.mockResolvedValue(shared);
    const user = userEvent.setup();
    await inPage();
    await user.click(screen.getByRole("button", { name: "Share my profile" }));
    const copy = await screen.findByRole("button", { name: "Copy link" });
    const sorry = "Plenipo couldn't copy it. Select the link and copy it yourself.";
    // The clipboard says no.
    clipboard(vi.fn().mockRejectedValue(new Error("denied")));
    await user.click(copy);
    expect(await screen.findByText(sorry)).toBeVisible();
    // There is no clipboard at all.
    Reflect.deleteProperty(navigator, "clipboard");
    await user.click(screen.getByRole("button", { name: "Share my profile" }));
    await user.click(await screen.findByRole("button", { name: "Copy link" }));
    expect(await screen.findByText(sorry)).toBeVisible();
    expect(screen.getByText(link)).toBeVisible();
  });

  it("says in plain words when there is nothing to share yet", async () => {
    api.shareMyCommunityProfile.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "Join Community first."),
    );
    const user = userEvent.setup();
    await inPage();
    await user.click(screen.getByRole("button", { name: "Share my profile" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Join Community first.");
    expect(screen.queryByRole("img", { name: /Picture code/ })).not.toBeInTheDocument();
  });
});

describe("A card", () => {
  const article = (card: CardView) => {
    render(<CommunityCard card={card} />);
    return screen.getByRole("article");
  };

  it("shows every part that is there, in plain words", () => {
    const card = article(
      cardView({
        displayName: "Pat Lee",
        status: "busy",
        mood: "focused",
        message: "Back at 3",
        company: "Lee Builders",
        businessKinds: ["construction", "trades"],
        businessLine: "Kitchens and decks",
        region: "US-CA",
        badges: ["helper", "founding_member", "top_helper"],
        points: 1250,
        thankedBy: 12,
      }),
    );
    expect(card).toHaveAccessibleName("@pat-lee");
    const inCard = within(card);
    expect(inCard.getByText("Pat Lee")).toBeInTheDocument();
    expect(inCard.getByText("@pat-lee")).toBeInTheDocument();
    expect(inCard.getByText("Busy")).toBeInTheDocument();
    expect(inCard.getByText("Focused")).toBeInTheDocument();
    expect(inCard.getByText("Back at 3")).toBeInTheDocument();
    expect(inCard.getByText("Lee Builders")).toBeInTheDocument();
    expect(inCard.getByText("Construction, Trades")).toBeInTheDocument();
    expect(inCard.getByText("Kitchens and decks")).toBeInTheDocument();
    expect(inCard.getByText("California (United States)")).toBeInTheDocument();
    const badges = inCard.getByRole("list", { name: "Badges" });
    expect(
      within(badges)
        .getAllByRole("listitem")
        .map((b) => b.textContent),
    ).toEqual(["Helper", "Founding member", "Top helper this week"]);
    expect(inCard.getByText("1,250 points")).toBeInTheDocument();
    expect(inCard.getByText("Thanked by 12 people")).toBeInTheDocument();
    // The status is a light and a word.
    expect(card.querySelector(".owner-light")).toHaveAttribute("data-status", "busy");
  });

  it("shows only the Community name and the points when a person hides the rest", () => {
    const card = article(
      cardView({ displayName: null, status: null, mood: null, businessKinds: [] }),
    );
    expect(card).toHaveTextContent(/^@pat-lee0 points$/);
    expect(within(card).queryByRole("list")).not.toBeInTheDocument();
  });

  it("says Offline in words, with no light", () => {
    const card = article(cardView({ status: "offline" }));
    expect(within(card).getByText("Offline")).toBeInTheDocument();
    expect(card.querySelector(".owner-light")).toBeNull();
  });

  it("says point and person in the singular, and nothing for no thanks", () => {
    const { unmount } = render(<CommunityCard card={cardView({ points: 1, thankedBy: 1 })} />);
    expect(screen.getByText("1 point")).toBeInTheDocument();
    expect(screen.getByText("Thanked by 1 person")).toBeInTheDocument();
    unmount();
    render(<CommunityCard card={cardView({ points: 0, thankedBy: 0 })} />);
    expect(screen.getByText("0 points")).toBeInTheDocument();
    expect(screen.queryByText(/Thanked/)).not.toBeInTheDocument();
  });

  it("leaves out a status, mood, badge, kind, or place this copy of Plenipo does not know", () => {
    const card = article(
      cardView({
        status: "doNotDisturb",
        mood: "constructor",
        badges: ["future_badge", "helper"],
        businessKinds: ["space_travel"],
        region: null,
      }),
    );
    expect(card).toHaveTextContent(/^Pat Lee@pat-leeHelper0 points$/);
    expect(card.querySelector(".people-card__tile")).toBeNull();
    expect(card.querySelector(".people-card__business")).toBeNull();
  });

  it("never shows a hidden character as it is: each becomes a mark", () => {
    const card = article(
      cardView({
        name: "pat\u202elee",
        displayName: "Pat\u0007Lee",
        message: "hello\u202eevil\n\u2066more\u2069",
        company: "Lee\u200bBuilders\u202c",
        businessLine: "line\u2028two\u0000",
        region: "ZZ\u202e",
      }),
    );
    expect(card.textContent).not.toMatch(HIDDEN);
    expect(card.textContent).toContain("Pat\uFFFDLee");
    expect(card.textContent).toContain("hello\uFFFDevil\uFFFD\uFFFDmore\uFFFD");
    expect(card.textContent).toContain("Lee\uFFFDBuilders\uFFFD");
    expect(card.textContent).toContain("@pat\uFFFDlee");
    // The name that names the card is as safe as the one shown.
    expect(card).toHaveAccessibleName("@pat\uFFFDlee");
  });

  it("never shows a field as a web page: it is text, with no picture, link, or button", () => {
    const card = article(
      cardView({
        displayName: "<b>Pat</b>",
        message: "<script>alert(1)</script> see https://evil.example/x",
        company: "<img src=x onerror=alert(1)>",
        businessLine: '<a href="https://evil.example">click</a>',
      }),
    );
    expect(within(card).getByText("<img src=x onerror=alert(1)>")).toBeInTheDocument();
    expect(within(card).getByText("<b>Pat</b>")).toBeInTheDocument();
    expect(card.textContent).toContain("<script>alert(1)</script> see https://evil.example/x");
    // No element came out of any of it (the person-shaped mark is an icon, not a picture).
    expect(card.querySelector("img, script, b, a, [onerror], [href]")).toBeNull();
    expect(within(card).queryAllByRole("link")).toHaveLength(0);
    expect(within(card).queryAllByRole("button")).toHaveLength(0);
  });
});

describe("A card's picture", () => {
  const withPicture = (patch: Partial<CardView> = {}) =>
    cardView({ hasPicture: true, pictureVersion: "v1", ...patch });
  const pictures = (root: ParentNode = document) => [...root.querySelectorAll("img")];

  it("is asked for only when the card says it has one", async () => {
    render(<CommunityCard card={cardView({ hasPicture: false, pictureVersion: "v1" })} />);
    await act(() => Promise.resolve());
    expect(api.communityPicture).not.toHaveBeenCalled();
    expect(pictures()).toHaveLength(0);
  });

  it("is shown only as a data:image/png;base64 address", async () => {
    api.communityPicture.mockResolvedValue(PNG);
    render(<CommunityCard card={withPicture()} />);
    await waitFor(() => expect(pictures()).toHaveLength(1));
    expect(api.communityPicture).toHaveBeenCalledWith(cardView().memberId);
    const [img] = pictures();
    expect(img?.getAttribute("src")).toBe(`data:image/png;base64,${PNG}`);
    expect(img).toHaveAttribute("alt", "");
  });

  it("is asked for once for each member and picture version", async () => {
    api.communityPicture.mockResolvedValue(PNG);
    const first = render(<CommunityCard card={withPicture()} />);
    await waitFor(() => expect(pictures()).toHaveLength(1));
    // Scrolling away and back (the card mounts again): no new request, and no flash.
    first.unmount();
    const again = render(<CommunityCard card={withPicture()} />);
    expect(pictures()).toHaveLength(1);
    // The same card shown again in another list, at the same time.
    render(<CommunityCard card={withPicture()} />);
    await act(() => Promise.resolve());
    expect(api.communityPicture).toHaveBeenCalledTimes(1);
    // A new picture version is a new picture.
    again.rerender(<CommunityCard card={withPicture({ pictureVersion: "v2" })} />);
    await waitFor(() => expect(api.communityPicture).toHaveBeenCalledTimes(2));
    // Another member is another picture.
    render(<CommunityCard card={withPicture({ memberId: mid(2), name: "kim-ode" })} />);
    await waitFor(() => expect(api.communityPicture).toHaveBeenCalledTimes(3));
    expect(api.communityPicture).toHaveBeenLastCalledWith(mid(2));
    for (const img of pictures()) {
      expect(img.getAttribute("src")?.startsWith("data:image/png;base64,")).toBe(true);
    }
  });

  it("asks once for cards shown together", async () => {
    api.communityPicture.mockResolvedValue(PNG);
    render(
      <>
        <CommunityCard card={withPicture()} />
        <CommunityCard card={withPicture()} />
      </>,
    );
    await waitFor(() => expect(pictures()).toHaveLength(2));
    expect(api.communityPicture).toHaveBeenCalledTimes(1);
  });

  it.each([
    ["not a PNG", "AAAAAAAAAAAAAAAA"],
    ["not plain base64", `${PNG}" onerror="alert(1)`],
    ["another address", "https://evil.example/x.png"],
    ["a data address of its own", `data:image/png;base64,${PNG}`],
    ["an empty answer", ""],
  ])("is not shown when it is %s", async (_, text) => {
    api.communityPicture.mockResolvedValue(text);
    render(<CommunityCard card={withPicture()} />);
    await waitFor(() => expect(api.communityPicture).toHaveBeenCalledTimes(1));
    await act(() => Promise.resolve());
    expect(pictures()).toHaveLength(0);
  });

  it("leaves the person-shaped mark when there is none, or it can't come, and asks again later", async () => {
    api.communityPicture.mockResolvedValueOnce(null);
    const none = render(<CommunityCard card={withPicture()} />);
    await waitFor(() => expect(api.communityPicture).toHaveBeenCalledTimes(1));
    await act(() => Promise.resolve());
    expect(pictures()).toHaveLength(0);
    expect(none.container.querySelector(".people-card__picture--none")).not.toBeNull();
    none.unmount();

    forgetPictures();
    api.communityPicture.mockRejectedValueOnce(new commands.PlenipoCommandError("internal", "no"));
    api.communityPicture.mockResolvedValueOnce(PNG);
    const failed = render(<CommunityCard card={withPicture()} />);
    await waitFor(() => expect(api.communityPicture).toHaveBeenCalledTimes(2));
    await act(() => Promise.resolve());
    expect(pictures()).toHaveLength(0);
    // A failure is not kept: the next card asks again.
    failed.unmount();
    render(<CommunityCard card={withPicture()} />);
    await waitFor(() => expect(pictures()).toHaveLength(1));
    expect(api.communityPicture).toHaveBeenCalledTimes(3);
  });

  it("shows no picture on the cards of a search that has no picture parts", async () => {
    api.communityDirectory.mockResolvedValue(
      peoplePage([cardView({ memberId: mid(1), hasPicture: false })]),
    );
    const user = userEvent.setup();
    await inPage();
    await user.click(screen.getByRole("button", { name: "Search" }));
    await part("Directory").findByRole("article");
    expect(api.communityPicture).not.toHaveBeenCalled();
  });
});

describe("The words on a card", () => {
  it("has a word for every badge in the contract, and no other", () => {
    // The wire values are the contract's own (`Badge`): a drift would hide a badge.
    const schema = JSON.parse(
      readFileSync(
        resolve(process.cwd(), "../../contracts/community/v1/schema/community.schema.json"),
        "utf8",
      ),
    ) as { $defs: { Badge: { enum: string[] } } };
    expect([...BADGE_WORDS.keys()]).toEqual(schema.$defs.Badge.enum);
    expect([...BADGE_WORDS.values()]).toEqual([
      "Founding member",
      "Helper",
      "Connector",
      "Good neighbor",
      "Trusted",
      "Top helper this week",
    ]);
  });
});
