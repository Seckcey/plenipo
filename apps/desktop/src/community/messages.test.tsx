import { SYSTEM_WORDS, type CommunityView, type ConversationSummary } from "@plenipo/types";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { ATTACH_EVENT } from "../files/refs";
import { setSystemWords } from "../system/words";
import { a11yProblems } from "../test/a11y";
import {
  cardView,
  communityView,
  conversationSummary,
  conversationView,
  messageView,
} from "../test/communityFixtures";
import { emptyOrganization, sampleOrganization } from "../test/orgFixtures";
import { QUIET_MS } from "./Conversation";
import { OUTSIDE_WORDS, REACTIONS } from "./messageWords";
import { PeoplePage } from "./PeoplePage";
import { forgetPictures } from "./pictures";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getCommunity: vi.fn(),
    findInCommunity: vi.fn(),
    communityDirectory: vi.fn(),
    communityPicture: vi.fn(),
    communityConversations: vi.fn(),
    communityConversation: vi.fn(),
    sendCommunityMessage: vi.fn(),
    reactInCommunity: vi.fn(),
    acceptCommunityRequest: vi.fn(),
    leaveCommunityConversation: vi.fn(),
    deleteCommunityMessage: vi.fn(),
    communitySafetyCodeChecked: vi.fn(),
    openCommunityLink: vi.fn(),
    giveCommunityMessageToWorker: vi.fn(),
    getOrganization: vi.fn(),
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

/** A member ID and a message ID in the contract's forms. */
const mid = (n: number) => `cm_${String(n).padStart(26, "0")}`;
const iid = (n: number) => `ci_${String(n).padStart(26, "0")}`;

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

const PAT = conversationSummary({ memberId: mid(1), name: "pat-lee", displayName: "Pat Lee" });
const KIM = conversationSummary({
  memberId: mid(2),
  name: "kim-ode",
  displayName: "Kim Ode",
  state: "requestedByThem",
});

/** Every character a person cannot see (or that can disguise words), as a message must not show. */
// eslint-disable-next-line no-control-regex
const HIDDEN = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f\u200b-\u200f\u2028-\u202e\u2060-\u2069]/;

beforeEach(() => {
  vi.clearAllMocks();
  // What one test said a command answers is not what the next one hears.
  for (const command of [
    api.getCommunity,
    api.findInCommunity,
    api.communityDirectory,
    api.sendCommunityMessage,
    api.reactInCommunity,
    api.acceptCommunityRequest,
    api.leaveCommunityConversation,
    api.deleteCommunityMessage,
    api.communitySafetyCodeChecked,
    api.openCommunityLink,
    api.giveCommunityMessageToWorker,
    api.getOrganization,
  ]) {
    command.mockReset();
  }
  forgetPictures();
  heard.current = () => undefined;
  api.communityConversations.mockResolvedValue([PAT]);
  api.communityConversation.mockResolvedValue(conversationView(PAT, [messageView()]));
});

afterEach(() => {
  vi.restoreAllMocks();
  setSystemWords(SYSTEM_WORDS.windows);
});

type User = ReturnType<typeof userEvent.setup>;

/** The Community section, on its Messages tab. */
async function inMessages(view: CommunityView = signedIn) {
  api.getCommunity.mockResolvedValue(view);
  const user = userEvent.setup();
  const shown = render(<PeoplePage go={go} />);
  await user.click(await screen.findByRole("tab", { name: /Messages/ }));
  return { user, ...shown };
}

/** Open a conversation from the list (the row named by its Community name). */
async function openRow(user: User, name: string, title: string | RegExp) {
  await user.click(await screen.findByRole("button", { name: new RegExp(`@${name}`) }));
  return screen.findByRole("heading", { level: 2, name: title });
}

/** The Messages tab with Pat's conversation open. */
async function inPat(conversation = conversationView(PAT, [messageView()]), view = signedIn) {
  api.communityConversation.mockResolvedValue(conversation);
  const shown = await inMessages(view);
  // With the name on his card, or with only the @name.
  await openRow(shown.user, "pat-lee", /^(Pat Lee|@pat-lee)$/);
  await screen.findByText(/^Sealed: only you and /);
  return shown;
}

const box = (name = "Message to Pat Lee") => screen.getByRole("textbox", { name });
const items = () => within(screen.getByRole("list", { name: "Messages" })).getAllByRole("listitem");
const dialog = (name: string) => screen.findByRole("dialog", { name });

describe("The Messages tab", () => {
  const list: ConversationSummary[] = [
    conversationSummary({ ...PAT, unseen: 2, lastAt: 300 }),
    conversationSummary({ ...KIM, unseen: 1, lastAt: 400 }),
    conversationSummary({
      memberId: mid(3),
      name: "sam-ray",
      displayName: null,
      lastAt: 500,
      computersChanged: true,
    }),
    // A person with no conversation is not in the list.
    conversationSummary({ memberId: mid(4), name: "no-talk", state: "none", lastAt: 600 }),
    conversationSummary({
      memberId: mid(5),
      name: "left-me",
      displayName: null,
      state: "leftByMe",
      lastAt: 100,
    }),
  ];

  it("sits beside People, with how many messages are new on its tab", async () => {
    api.communityConversations.mockResolvedValue(list);
    api.getCommunity.mockResolvedValue(signedIn);
    render(<PeoplePage go={go} />);
    // The number is there while People is open.
    const tab = await screen.findByRole("tab", { name: /Messages/ });
    expect(await within(tab).findByRole("img", { name: "3 unseen" })).toHaveTextContent("3");
    expect(screen.getAllByRole("tab").map((t) => t.textContent)).toEqual(["People", "Messages3"]);
  });

  it("has no number on the tab when nothing is new", async () => {
    api.communityConversations.mockResolvedValue([PAT]);
    api.getCommunity.mockResolvedValue(signedIn);
    render(<PeoplePage go={go} />);
    const tab = await screen.findByRole("tab", { name: "Messages" });
    await screen.findByRole("heading", { name: "Find someone" });
    expect(within(tab).queryByRole("img")).not.toBeInTheDocument();
  });

  it("lists conversations newest first, and Requests in a group of their own", async () => {
    api.communityConversations.mockResolvedValue(list);
    await inMessages();
    const requests = await screen.findByRole("list", { name: "Requests" });
    expect(within(requests).getAllByRole("button")).toHaveLength(1);
    expect(screen.getByRole("heading", { level: 2, name: "Requests" })).toBeInTheDocument();
    const rows = within(screen.getByRole("list", { name: "Conversations" })).getAllByRole("button");
    expect(rows.map((r) => r.textContent)).toEqual([
      "@sam-ray@sam-ray's computers changed",
      "Pat Lee@pat-lee2",
      "@left-meYou left",
    ]);
    expect(screen.queryByText("@no-talk")).not.toBeInTheDocument();
  });

  it("shows @name, the name on the card, how many are new, and the computers-changed mark", async () => {
    api.communityConversations.mockResolvedValue(list);
    await inMessages();
    const kim = await screen.findByRole("button", { name: /@kim-ode/ });
    expect(within(kim).getByText("Kim Ode")).toBeInTheDocument();
    expect(within(kim).getByText("@kim-ode")).toBeInTheDocument();
    expect(within(kim).getByRole("img", { name: "1 unseen" })).toBeInTheDocument();
    const sam = screen.getByRole("button", { name: /@sam-ray/ });
    expect(within(sam).getByText("@sam-ray's computers changed")).toBeInTheDocument();
    expect(within(screen.getByRole("button", { name: /@pat-lee/ })).queryByText(/computers/)).toBe(
      null,
    );
  });

  it("says so, and where to start, when there are none", async () => {
    api.communityConversations.mockResolvedValue([]);
    await inMessages();
    expect(
      await screen.findByText(
        "No conversations yet. Find someone on the People tab, then press Message.",
      ),
    ).toBeVisible();
    expect(screen.queryByRole("heading", { name: "Requests" })).not.toBeInTheDocument();
    expect(screen.getByText("Pick a conversation to read it.")).toBeInTheDocument();
  });

  it("says in plain words when the list can't be read", async () => {
    api.communityConversations.mockRejectedValue(
      new commands.PlenipoCommandError("internal", "Plenipo is still starting."),
    );
    await inMessages();
    expect(await screen.findByRole("alert")).toHaveTextContent("Plenipo is still starting.");
  });

  it("opens a conversation: it reads it, and shows who it is with", async () => {
    api.communityConversations.mockResolvedValue(list);
    const { user } = await inMessages();
    await openRow(user, "pat-lee", "Pat Lee");
    expect(api.communityConversation).toHaveBeenCalledWith(mid(1), null);
    expect(screen.getByText("@pat-lee", { selector: ".conversation__handle" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /@pat-lee/ })).toHaveAttribute(
      "aria-current",
      "true",
    );
    // Only the one that was opened is read.
    expect(api.communityConversation).toHaveBeenCalledTimes(1);
  });

  it("has a way back to the list (shown when the window is narrow)", async () => {
    const { user } = await inPat();
    await user.click(screen.getByRole("button", { name: "All messages" }));
    expect(screen.getByText("Pick a conversation to read it.")).toBeInTheDocument();
  });

  it("passes the accessibility smoke check", async () => {
    api.communityConversations.mockResolvedValue(list);
    const { user, container } = await inMessages();
    await openRow(user, "pat-lee", "Pat Lee");
    await screen.findByRole("list", { name: "Messages" });
    expect(a11yProblems(container)).toEqual([]);
  });
});

describe("The open conversation", () => {
  it("says it is sealed, in bold, with the person's name", async () => {
    await inPat();
    expect(screen.getByText("Sealed: only you and Pat Lee can read this").tagName).toBe("STRONG");
  });

  it("calls a person with no name on the card by their @name", async () => {
    api.communityConversations.mockResolvedValue([{ ...PAT, displayName: null }]);
    await inPat(conversationView({ ...PAT, displayName: null }, [messageView()]));
    expect(screen.getByRole("heading", { level: 2, name: "@pat-lee" })).toBeInTheDocument();
    expect(screen.getByText("Sealed: only you and @pat-lee can read this")).toBeInTheDocument();
  });

  it("shows messages oldest first, with who wrote them and when", async () => {
    await inPat(
      conversationView(PAT, [
        messageView({
          itemId: iid(1),
          text: "First",
          acceptedAt: 1_790_000_100,
          sentAt: 1_790_000_000,
        }),
        messageView({
          itemId: iid(2),
          text: "Second",
          outgoing: true,
          state: "delivered",
          acceptedAt: null,
          sentAt: 1_790_000_200,
        }),
      ]),
    );
    const [first, second] = items();
    expect(first).toHaveTextContent("First");
    expect(within(first as HTMLElement).getByText("Pat Lee")).toBeInTheDocument();
    expect(second).toHaveTextContent("Second");
    expect(within(second as HTMLElement).getByText("You")).toBeInTheDocument();
    // The time 8 West took it, else the time it was sent.
    expect(first?.querySelector("time")).toHaveAttribute(
      "datetime",
      new Date(1_790_000_100 * 1000).toISOString(),
    );
    expect(second?.querySelector("time")).toHaveAttribute(
      "datetime",
      new Date(1_790_000_200 * 1000).toISOString(),
    );
    expect(first?.querySelector("time")?.textContent).not.toBe("");
  });

  it.each([
    ["waiting", "Waiting to be delivered"],
    ["delivered", "Delivered"],
    ["notDelivered", "Not delivered"],
  ])("says where a message of yours is: %s", async (state, words) => {
    await inPat(conversationView(PAT, [messageView({ outgoing: true, state, reportable: false })]));
    expect(within(items()[0] as HTMLElement).getByText(words)).toBeInTheDocument();
  });

  it("says nothing about delivery for their messages", async () => {
    await inPat();
    expect(within(items()[0] as HTMLElement).queryByText(/delivered/i)).not.toBeInTheDocument();
  });

  it("shows a GIF and a sticker as words, and offers no GIF or sticker button", async () => {
    await inPat(
      conversationView(PAT, [
        messageView({ itemId: iid(1), text: null, hasGif: true }),
        messageView({ itemId: iid(2), text: null, hasSticker: true }),
      ]),
    );
    expect(screen.getByText("A GIF (this version doesn't show GIFs)")).toBeInTheDocument();
    expect(screen.getByText("A sticker")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /GIF|sticker/i })).not.toBeInTheDocument();
    // A message with no words has nothing to give to a worker.
    expect(screen.queryByRole("button", { name: "Give to a worker" })).not.toBeInTheDocument();
  });

  it("says when a message has nothing this version can show", async () => {
    await inPat(conversationView(PAT, [messageView({ text: null })]));
    expect(
      screen.getByText("This message can't be shown in this version of Plenipo."),
    ).toBeVisible();
  });

  it("shows reactions under a message, and marks yours", async () => {
    await inPat(
      conversationView(PAT, [
        messageView({
          reactions: [
            { emoji: "\u{1F44D}", mine: true },
            { emoji: "\u{1F602}", mine: false },
          ],
        }),
      ]),
    );
    const reactions = within(items()[0] as HTMLElement).getByRole("list", { name: "Reactions" });
    const [mine, theirs] = within(reactions).getAllByRole("listitem");
    expect(mine).toHaveTextContent("\u{1F44D}You");
    expect(theirs).toHaveTextContent(/^\u{1F602}$/u);
  });

  it("loads older messages from before the oldest one shown", async () => {
    api.communityConversation
      .mockResolvedValueOnce(
        conversationView(PAT, [
          messageView({ itemId: iid(5), text: "Newer", acceptedAt: 1_790_000_500 }),
          messageView({ itemId: iid(6), text: "Newest", acceptedAt: 1_790_000_600 }),
        ]),
      )
      .mockResolvedValueOnce(
        conversationView(PAT, [
          messageView({ itemId: iid(3), text: "Old", acceptedAt: 1_790_000_300 }),
          messageView({ itemId: iid(4), text: "Older", acceptedAt: 1_790_000_400 }),
        ]),
      );
    const { user } = await inMessages();
    await openRow(user, "pat-lee", "Pat Lee");
    await screen.findByText("Newest");
    await user.click(screen.getByRole("button", { name: "Load older" }));
    // The page before the oldest message shown, by its ID, so one sharing its second isn't skipped.
    expect(api.communityConversation).toHaveBeenLastCalledWith(mid(1), iid(5));
    await screen.findByText("Older");
    expect(items().map((i) => i.querySelector(".message__text")?.textContent)).toEqual([
      "Old",
      "Older",
      "Newer",
      "Newest",
    ]);
  });

  it("stops offering older messages when there are none", async () => {
    api.communityConversation
      .mockResolvedValueOnce(conversationView(PAT, [messageView({ text: "Only one" })]))
      .mockResolvedValueOnce(conversationView(PAT, []));
    const { user } = await inMessages();
    await openRow(user, "pat-lee", "Pat Lee");
    await user.click(await screen.findByRole("button", { name: "Load older" }));
    expect(await screen.findByText("That is every message that is kept here.")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Load older" })).not.toBeInTheDocument();
  });

  it("says in plain words when a conversation can't be read, and tries again", async () => {
    api.communityConversation.mockRejectedValueOnce(
      new commands.PlenipoCommandError("internal", "Plenipo is still starting."),
    );
    const { user } = await inMessages();
    await user.click(await screen.findByRole("button", { name: /@pat-lee/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Plenipo is still starting.");
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Hello")).toBeVisible();
  });

  it("has Report and Block in its header, and Report on a message of theirs", async () => {
    await inPat();
    const header = screen.getByRole("heading", { level: 2, name: "Pat Lee" }).closest("header");
    expect(within(header as HTMLElement).getByRole("button", { name: "Report" })).toBeVisible();
    expect(within(header as HTMLElement).getByRole("button", { name: "Block" })).toBeVisible();
    expect(within(items()[0] as HTMLElement).getByRole("button", { name: "Report" })).toBeVisible();
  });
});

describe("Writing", () => {
  const sent = messageView({ outgoing: true, state: "delivered", text: "Hi" });

  it("sends with Enter, then clears the box and reads the conversation again", async () => {
    api.sendCommunityMessage.mockResolvedValue(sent);
    const { user } = await inPat();
    await user.type(box(), "Hi there{Enter}");
    expect(api.sendCommunityMessage).toHaveBeenCalledWith(mid(1), "pat-lee", "Hi there", null);
    await waitFor(() => expect(box()).toHaveValue(""));
    await waitFor(() => expect(api.communityConversation.mock.calls.length).toBeGreaterThan(1));
  });

  it("sends with the Send button, which waits for words", async () => {
    api.sendCommunityMessage.mockResolvedValue(sent);
    const { user } = await inPat();
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
    await user.type(box(), "   ");
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
    await user.type(box(), "Hello");
    await user.click(screen.getByRole("button", { name: "Send" }));
    // The spaces around the words are not sent.
    expect(api.sendCommunityMessage).toHaveBeenCalledWith(mid(1), "pat-lee", "Hello", null);
  });

  it("starts a new line with Shift+Enter, and does not send", async () => {
    const { user } = await inPat();
    await user.type(box(), "one{Shift>}{Enter}{/Shift}two");
    expect(box()).toHaveValue("one\ntwo");
    expect(api.sendCommunityMessage).not.toHaveBeenCalled();
  });

  it("does not send while a letter in another language is being composed", async () => {
    const { user } = await inPat();
    await user.type(box(), "x");
    fireEvent.keyDown(box(), { key: "Enter", isComposing: true });
    expect(api.sendCommunityMessage).not.toHaveBeenCalled();
  });

  it("counts characters, and says what Enter does", async () => {
    const { user } = await inPat();
    expect(screen.getByText("0 of 4000")).toBeInTheDocument();
    expect(screen.getByText("Enter sends. Shift+Enter starts a new line.")).toBeInTheDocument();
    await user.type(box(), "abc");
    expect(screen.getByText("3 of 4000")).toBeInTheDocument();
    expect(box()).toHaveAccessibleDescription(
      "3 of 4000 Enter sends. Shift+Enter starts a new line.",
    );
  });

  it("holds at most 4000 characters", async () => {
    const { user } = await inPat();
    await user.click(box());
    await user.paste("x".repeat(4100));
    expect(box()).toHaveValue("x".repeat(4000));
    expect(screen.getByText("4000 of 4000")).toBeInTheDocument();
  });

  it("counts an emoji as one character, as 8 West does", async () => {
    const { user } = await inPat();
    await user.click(box());
    await user.paste("\u{1F600}".repeat(4001));
    expect(Array.from((box() as HTMLTextAreaElement).value)).toHaveLength(4000);
    expect(screen.getByText("4000 of 4000")).toBeInTheDocument();
  });

  it("keeps every language and emoji as it was typed", async () => {
    api.sendCommunityMessage.mockResolvedValue(sent);
    const { user } = await inPat();
    await user.click(box());
    await user.paste("مرحبا 👋 こんにちは");
    await user.keyboard("{Enter}");
    expect(api.sendCommunityMessage).toHaveBeenCalledWith(
      mid(1),
      "pat-lee",
      "مرحبا 👋 こんにちは",
      null,
    );
  });

  it("shows a problem in plain words and keeps what was written", async () => {
    api.sendCommunityMessage.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "8 West can't be reached right now."),
    );
    const { user } = await inPat();
    await user.type(box(), "Hi{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "8 West can't be reached right now.",
    );
    expect(box()).toHaveValue("Hi");
    // Typing goes on; the next send is tried again.
    api.sendCommunityMessage.mockResolvedValue(sent);
    await user.keyboard("{Enter}");
    await waitFor(() => expect(box()).toHaveValue(""));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows the words of Part of Pro, with the way to Settings → License", async () => {
    const words = "Starting a conversation, a link, or an invitation in Community is part of Pro.";
    api.sendCommunityMessage.mockRejectedValue(
      new commands.PlenipoCommandError("partOfPro", words),
    );
    const { user } = await inPat();
    await user.type(box(), "Hi{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent(words);
    expect(box()).toHaveValue("Hi");
    await user.click(screen.getByRole("button", { name: "Open Settings → License" }));
    expect(go).toHaveBeenCalledWith({ view: "settings", id: "license" });
  });

  it("has no GIF button and no sticker button", async () => {
    await inPat();
    const form = screen.getByRole("form", { name: "Write a message" });
    expect(
      within(form)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Send"]);
  });
});

describe("A picture or a file", () => {
  const PHOTOS = "Photos can't be sent in Community";
  /** A file, and the ways to read it, each one a spy: a read would show. */
  function untouchable(name: string, type: string) {
    const file = new File(["not really what it says"], name, { type });
    const reads = ["arrayBuffer", "text", "stream", "slice"].map((way) => {
      const spy = vi.fn();
      Object.defineProperty(file, way, { value: spy, configurable: true });
      return spy;
    });
    return { file, reads };
  }

  it("pasted is refused, is never read, and nothing is sent", async () => {
    const { file, reads } = untouchable("me.png", "image/png");
    const { user } = await inPat();
    const clipboardData = {
      files: [file],
      items: [{ kind: "file", type: "image/png", getAsFile: () => file }],
      types: ["Files"],
      getData: () => "",
    };
    // `fireEvent` says false when the page cancelled the paste.
    expect(fireEvent.paste(box(), { clipboardData })).toBe(false);
    expect(await screen.findByRole("alert")).toHaveTextContent(PHOTOS);
    expect(screen.getByRole("alert")).toHaveTextContent(/^Photos can't be sent in Community$/);
    expect(box()).toHaveValue("");
    await user.keyboard("{Enter}");
    expect(api.sendCommunityMessage).not.toHaveBeenCalled();
    for (const spy of reads) expect(spy).not.toHaveBeenCalled();
  });

  it("dropped is refused, is never read, and nothing is sent", async () => {
    const { file, reads } = untouchable("plan.pdf", "application/pdf");
    await inPat();
    const form = screen.getByRole("form", { name: "Write a message" });
    const dataTransfer = {
      files: [file],
      items: [{ kind: "file", type: file.type }],
      types: ["Files"],
    };
    // The page lets the file land (so the window does not open it), then turns it away.
    expect(fireEvent.dragOver(form, { dataTransfer })).toBe(false);
    expect(fireEvent.drop(form, { dataTransfer })).toBe(false);
    expect(await screen.findByRole("alert")).toHaveTextContent(PHOTOS);
    expect(api.sendCommunityMessage).not.toHaveBeenCalled();
    for (const spy of reads) expect(spy).not.toHaveBeenCalled();
  });

  it("dropped from File Explorer or the Files panel is refused the same way", async () => {
    await inPat();
    const form = screen.getByRole("form", { name: "Write a message" });
    act(() => {
      form.dispatchEvent(new CustomEvent(ATTACH_EVENT, { detail: [] }));
    });
    expect(await screen.findByRole("alert")).toHaveTextContent(PHOTOS);
    expect(api.sendCommunityMessage).not.toHaveBeenCalled();
  });

  it("goes away with the next thing typed, and words can still be pasted", async () => {
    const { user } = await inPat();
    fireEvent.paste(box(), {
      clipboardData: {
        files: [untouchable("me.png", "image/png").file],
        items: [],
        types: ["Files"],
        getData: () => "",
      },
    });
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    await user.click(box());
    await user.paste("some words");
    expect(box()).toHaveValue("some words");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});

describe("Other people's words", () => {
  /** The message's own box. */
  const only = () => items()[0] as HTMLElement;

  it("shows a hidden character as a visible mark, and never as it is", async () => {
    await inPat(
      conversationView(PAT, [messageView({ text: "Send it to \u202Eevil\u200B\u0007 now" })]),
    );
    const message = only();
    expect(message.textContent).not.toMatch(HIDDEN);
    const marks = [...message.querySelectorAll(".message__mark")];
    expect(marks.map((m) => m.textContent)).toEqual([
      "\u2039right-to-left override\u203A",
      "\u2039zero-width space\u203A",
      "\u2039control character\u203A",
    ]);
    expect(marks[0]).toHaveAttribute("title", "right-to-left override (U+202E)");
    // The words around them are all still there.
    expect(message.querySelector(".message__text")?.textContent).toContain("Send it to ");
    expect(message.querySelector(".message__text")?.textContent).toContain("evil");
    expect(message.querySelector(".message__text")?.textContent).toContain(" now");
  });

  it("shows web page code as plain text, with no new element", async () => {
    const words = "<img src=x onerror=alert(1)> <b>bold</b> <script>alert(2)</script>";
    await inPat(conversationView(PAT, [messageView({ text: words })]));
    const message = only();
    expect(message.querySelector(".message__text")?.textContent).toBe(words);
    expect(message.querySelector("img, b, script, [onerror], a, [href]")).toBeNull();
    expect(within(message).queryAllByRole("link")).toHaveLength(0);
    expect(within(message).queryByText("bold")).not.toBeInTheDocument();
  });

  it("keeps the line breaks a person wrote", async () => {
    await inPat(conversationView(PAT, [messageView({ text: "one\ntwo\n\nthree" })]));
    expect(only().querySelector(".message__text")?.textContent).toBe("one\ntwo\n\nthree");
  });

  it("shows a name in the list on one line, with marks for hidden characters", async () => {
    api.communityConversations.mockResolvedValue([
      { ...PAT, name: "pat\u202Elee", displayName: "Pat\nLee\u200B" },
    ]);
    await inMessages();
    const row = await screen.findByRole("button", { name: /@pat/ });
    expect(row.textContent).not.toMatch(HIDDEN);
    expect(row.textContent).not.toContain("\n");
    expect(row.textContent).toContain("Pat\uFFFDLee\uFFFD");
    expect(row.textContent).toContain("@pat\uFFFDlee");
  });

  it("shows a link as text with an Open link button, never as a link", async () => {
    await inPat(
      conversationView(PAT, [messageView({ text: "See https://example.com/a?b=1 for the plan." })]),
    );
    const message = only();
    const address = within(message).getByText("https://example.com/a?b=1");
    expect(address.tagName).toBe("SPAN");
    expect(message.querySelector("a, [href]")).toBeNull();
    expect(within(message).getByRole("button", { name: "Open link" })).toBeInTheDocument();
    expect(message.querySelector(".message__text")?.textContent).toBe(
      "See https://example.com/a?b=1 Open link for the plan.",
    );
  });

  it("asks before it opens a link, and opens that address only after you say yes", async () => {
    api.openCommunityLink.mockResolvedValue(undefined);
    const { user } = await inPat(
      conversationView(PAT, [messageView({ text: "See https://example.com/a?b=1 for the plan." })]),
    );
    await user.click(screen.getByRole("button", { name: "Open link" }));
    const ask = await dialog("Open this link in your web browser?");
    expect(within(ask).getByText("https://example.com/a?b=1").tagName).toBe("CODE");
    expect(api.openCommunityLink).not.toHaveBeenCalled();
    // No: nothing opens.
    await user.click(within(ask).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(api.openCommunityLink).not.toHaveBeenCalled();
    // Yes: the exact address opens.
    await user.click(screen.getByRole("button", { name: "Open link" }));
    await user.click(
      within(await dialog("Open this link in your web browser?")).getByRole("button", {
        name: "Yes, open it",
      }),
    );
    expect(api.openCommunityLink).toHaveBeenCalledTimes(1);
    expect(api.openCommunityLink).toHaveBeenCalledWith("https://example.com/a?b=1");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("asks for each link on its own", async () => {
    api.openCommunityLink.mockResolvedValue(undefined);
    const { user } = await inPat(
      conversationView(PAT, [messageView({ text: "https://a.example/x and http://b.example/y" })]),
    );
    const [, second] = screen.getAllByRole("button", { name: "Open link" });
    await user.click(second as HTMLElement);
    const ask = await dialog("Open this link in your web browser?");
    await user.click(within(ask).getByRole("button", { name: "Yes, open it" }));
    expect(api.openCommunityLink).toHaveBeenCalledWith("http://b.example/y");
  });

  it("says why when the link could not be opened, and keeps asking", async () => {
    api.openCommunityLink.mockRejectedValue(
      new commands.PlenipoCommandError(
        "invalidInput",
        "Plenipo opens only web addresses that start with https://.",
      ),
    );
    const { user } = await inPat(
      conversationView(PAT, [messageView({ text: "http://example.com/ok" })]),
    );
    await user.click(screen.getByRole("button", { name: "Open link" }));
    const ask = await dialog("Open this link in your web browser?");
    await user.click(within(ask).getByRole("button", { name: "Yes, open it" }));
    expect(await within(ask).findByRole("alert")).toHaveTextContent(
      "Plenipo opens only web addresses that start with https://.",
    );
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
});

describe("Reactions", () => {
  it("are exactly the five of ADR-164, and nothing else", () => {
    expect(REACTIONS).toEqual(["\u{1F44D}", "\u2764\uFE0F", "\u{1F602}", "\u{1F62E}", "\u{1F64F}"]);
  });

  it("offers the five, and sends the one you pick", async () => {
    api.reactInCommunity.mockResolvedValue(undefined);
    const { user } = await inPat(conversationView(PAT, [messageView({ itemId: iid(7) })]));
    expect(screen.queryByRole("group", { name: "Pick a reaction" })).not.toBeInTheDocument();
    await user.click(within(items()[0] as HTMLElement).getByRole("button", { name: "React" }));
    const picker = screen.getByRole("group", { name: "Pick a reaction" });
    expect(
      within(picker)
        .getAllByRole("button")
        .map((b) => b.getAttribute("aria-label")),
    ).toEqual(REACTIONS.map((e) => `React with ${e}`));
    await user.click(within(picker).getByRole("button", { name: "React with \u{1F602}" }));
    expect(api.reactInCommunity).toHaveBeenCalledWith(iid(7), "\u{1F602}");
    // It was read again, to show it.
    await waitFor(() => expect(api.communityConversation.mock.calls.length).toBeGreaterThan(1));
    expect(screen.queryByRole("group", { name: "Pick a reaction" })).not.toBeInTheDocument();
  });

  it("takes yours back when you pick it again", async () => {
    api.reactInCommunity.mockResolvedValue(undefined);
    const { user } = await inPat(
      conversationView(PAT, [
        messageView({
          itemId: iid(7),
          reactions: [{ emoji: "\u2764\uFE0F", mine: true }],
        }),
      ]),
    );
    await user.click(screen.getByRole("button", { name: "React" }));
    const heart = screen.getByRole("button", { name: "React with \u2764\uFE0F" });
    expect(heart).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "React with \u{1F44D}" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    await user.click(heart);
    expect(api.reactInCommunity).toHaveBeenCalledWith(iid(7), null);
  });

  it("changes yours to another when you pick another", async () => {
    api.reactInCommunity.mockResolvedValue(undefined);
    const { user } = await inPat(
      conversationView(PAT, [
        messageView({ itemId: iid(7), reactions: [{ emoji: "\u{1F44D}", mine: true }] }),
      ]),
    );
    await user.click(screen.getByRole("button", { name: "React" }));
    await user.click(screen.getByRole("button", { name: "React with \u{1F64F}" }));
    expect(api.reactInCommunity).toHaveBeenCalledWith(iid(7), "\u{1F64F}");
  });

  it("says why in plain words when a reaction can't be sent", async () => {
    api.reactInCommunity.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "That message isn't on this computer."),
    );
    const { user } = await inPat();
    await user.click(screen.getByRole("button", { name: "React" }));
    await user.click(screen.getByRole("button", { name: "React with \u{1F44D}" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "That message isn't on this computer.",
    );
  });
});

describe("Reply", () => {
  it("answers a message: shows a short quote, sends with its ID, and then forgets it", async () => {
    api.sendCommunityMessage.mockResolvedValue(messageView({ outgoing: true }));
    const { user } = await inPat(
      conversationView(PAT, [messageView({ itemId: iid(7), text: "Can you come on Friday?" })]),
    );
    await user.click(screen.getByRole("button", { name: "Reply" }));
    const replying = screen.getByText("Replying to", { exact: false });
    expect(replying).toHaveTextContent("Replying to Can you come on Friday?");
    expect(within(replying).getByText("Can you come on Friday?").tagName).toBe("Q");
    expect(box()).toHaveFocus();
    await user.type(box(), "Yes{Enter}");
    expect(api.sendCommunityMessage).toHaveBeenCalledWith(mid(1), "pat-lee", "Yes", iid(7));
    await waitFor(() => expect(screen.queryByText(/Replying to/)).not.toBeInTheDocument());
  });

  it("can be cancelled", async () => {
    api.sendCommunityMessage.mockResolvedValue(messageView({ outgoing: true }));
    const { user } = await inPat();
    await user.click(screen.getByRole("button", { name: "Reply" }));
    await user.click(screen.getByRole("button", { name: "Cancel reply" }));
    expect(screen.queryByText(/Replying to/)).not.toBeInTheDocument();
    await user.type(box(), "Hi{Enter}");
    expect(api.sendCommunityMessage).toHaveBeenCalledWith(mid(1), "pat-lee", "Hi", null);
  });

  it("shows a short quote of the message a message answers, when it is loaded", async () => {
    await inPat(
      conversationView(PAT, [
        messageView({ itemId: iid(1), text: "A very long question ".repeat(10) }),
        messageView({ itemId: iid(2), text: "The answer", replyTo: iid(1), outgoing: true }),
        messageView({ itemId: iid(3), text: "Another", replyTo: iid(99) }),
        messageView({ itemId: iid(4), text: "A picture", hasGif: true, replyTo: iid(1) }),
      ]),
    );
    const [, answer, , gif] = items();
    const quote = within(answer as HTMLElement).getByText(/A very long question/);
    expect(quote.tagName).toBe("Q");
    expect(quote.textContent?.endsWith("…")).toBe(true);
    expect(quote.textContent?.length).toBeLessThan(90);
    expect(within(gif as HTMLElement).getByText(/A very long question/)).toBeInTheDocument();
    expect(
      within(items()[2] as HTMLElement).getByText("In reply to an earlier message"),
    ).toBeVisible();
  });
});

describe("Delete for me", () => {
  it("asks first, with the words of ADR-172, and deletes from this PC only after yes", async () => {
    api.deleteCommunityMessage.mockResolvedValue(undefined);
    const { user } = await inPat(conversationView(PAT, [messageView({ itemId: iid(7) })]));
    await user.click(screen.getByRole("button", { name: "Delete for me" }));
    const ask = await dialog("Delete this message?");
    expect(ask).toHaveTextContent(
      "This deletes it from this PC. Your other PCs, and Pat Lee, keep their copies.",
    );
    expect(api.deleteCommunityMessage).not.toHaveBeenCalled();
    await user.click(within(ask).getByRole("button", { name: "Cancel" }));
    expect(api.deleteCommunityMessage).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Delete for me" }));
    await user.click(
      within(await dialog("Delete this message?")).getByRole("button", { name: "Delete for me" }),
    );
    expect(api.deleteCommunityMessage).toHaveBeenCalledWith(iid(7));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await waitFor(() => expect(api.communityConversation.mock.calls.length).toBeGreaterThan(1));
  });

  it("says this Mac or this computer, and its other Macs or computers, on those systems", async () => {
    setSystemWords(SYSTEM_WORDS.mac);
    const { user, unmount } = await inPat();
    await user.click(screen.getByRole("button", { name: "Delete for me" }));
    expect(await dialog("Delete this message?")).toHaveTextContent(
      "This deletes it from this Mac. Your other Macs, and Pat Lee, keep their copies.",
    );
    unmount();
    setSystemWords(SYSTEM_WORDS.linux);
    const again = await inPat();
    await again.user.click(screen.getByRole("button", { name: "Delete for me" }));
    expect(await dialog("Delete this message?")).toHaveTextContent(
      "This deletes it from this computer. Your other computers, and Pat Lee, keep their copies.",
    );
  });

  it("says why in the window when it can't be done", async () => {
    api.deleteCommunityMessage.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "That message isn't on this computer."),
    );
    const { user } = await inPat();
    await user.click(screen.getByRole("button", { name: "Delete for me" }));
    const ask = await dialog("Delete this message?");
    await user.click(within(ask).getByRole("button", { name: "Delete for me" }));
    expect(await within(ask).findByRole("alert")).toHaveTextContent(
      "That message isn't on this computer.",
    );
  });
});

describe("Give to a worker", () => {
  it("picks a full-time position, asks what to do with it, and gives it", async () => {
    api.getOrganization.mockResolvedValue(sampleOrganization());
    api.giveCommunityMessageToWorker.mockResolvedValue({} as never);
    const { user } = await inPat(
      conversationView(PAT, [messageView({ itemId: iid(7), text: "Please send me a price list" })]),
    );
    await user.click(screen.getByRole("button", { name: "Give to a worker" }));
    const ask = await dialog("Give to a worker");
    expect(within(ask).getByText(OUTSIDE_WORDS)).toBeInTheDocument();
    expect(OUTSIDE_WORDS).toBe(
      "The worker gets the words marked as outside words, so it treats them as information, never as orders.",
    );
    expect(ask).toHaveTextContent("Please send me a price list");
    // The same positions that can be given an objective anywhere else: full-time, with an agent.
    const pick = await within(ask).findByRole("combobox", { name: "Give it to" });
    expect(
      within(pick)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual([
      "VP (VP)",
      "Engineering Manager (Manager)",
      "Website Supervisor (Supervisor)",
      "Campaign Supervisor (Supervisor)",
    ]);
    await user.selectOptions(pick, "p-web");
    await user.type(
      within(ask).getByRole("textbox", { name: "What should they do with it?" }),
      "Write a reply for me",
    );
    expect(api.giveCommunityMessageToWorker).not.toHaveBeenCalled();
    await user.click(within(ask).getByRole("button", { name: "Give it" }));
    expect(api.giveCommunityMessageToWorker).toHaveBeenCalledWith(
      iid(7),
      "p-web",
      "Write a reply for me",
    );
    expect(await screen.findByText(/Given to Website Supervisor/)).toBeVisible();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("gives with no note, and starts with the first position", async () => {
    api.getOrganization.mockResolvedValue(sampleOrganization());
    api.giveCommunityMessageToWorker.mockResolvedValue({} as never);
    const { user } = await inPat(conversationView(PAT, [messageView({ itemId: iid(7) })]));
    await user.click(screen.getByRole("button", { name: "Give to a worker" }));
    const ask = await dialog("Give to a worker");
    await within(ask).findByRole("combobox", { name: "Give it to" });
    await user.click(within(ask).getByRole("button", { name: "Give it" }));
    expect(api.giveCommunityMessageToWorker).toHaveBeenCalledWith(iid(7), "p-super", "");
  });

  it("names positions with the rank names you chose", async () => {
    api.getOrganization.mockResolvedValue({ ...sampleOrganization(), titles: "army" });
    const { user } = await inPat();
    await user.click(screen.getByRole("button", { name: "Give to a worker" }));
    const pick = await within(await dialog("Give to a worker")).findByRole("combobox", {
      name: "Give it to",
    });
    expect(
      within(pick).getByRole("option", { name: "Website Supervisor (Sergeant)" }),
    ).toBeVisible();
  });

  it("says when no one can be given it yet", async () => {
    api.getOrganization.mockResolvedValue(emptyOrganization());
    const { user } = await inPat();
    await user.click(screen.getByRole("button", { name: "Give to a worker" }));
    const ask = await dialog("Give to a worker");
    expect(await within(ask).findByText(/No one can be given this yet/)).toBeVisible();
    expect(within(ask).queryByRole("combobox")).not.toBeInTheDocument();
    await user.click(within(ask).getByRole("button", { name: "Give it" }));
    expect(await within(ask).findByRole("alert")).toHaveTextContent("Pick who gets it first.");
    expect(api.giveCommunityMessageToWorker).not.toHaveBeenCalled();
  });

  it("says why in the window when it can't be given", async () => {
    api.getOrganization.mockResolvedValue(sampleOrganization());
    api.giveCommunityMessageToWorker.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "That message isn't on this computer."),
    );
    const { user } = await inPat();
    await user.click(screen.getByRole("button", { name: "Give to a worker" }));
    const ask = await dialog("Give to a worker");
    await within(ask).findByRole("combobox", { name: "Give it to" });
    await user.click(within(ask).getByRole("button", { name: "Give it" }));
    expect(await within(ask).findByRole("alert")).toHaveTextContent(
      "That message isn't on this computer.",
    );
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("says why when the positions can't be read", async () => {
    api.getOrganization.mockRejectedValue(
      new commands.PlenipoCommandError("internal", "Plenipo is still starting."),
    );
    const { user } = await inPat();
    await user.click(screen.getByRole("button", { name: "Give to a worker" }));
    expect(await within(await dialog("Give to a worker")).findByRole("alert")).toHaveTextContent(
      "Plenipo is still starting.",
    );
  });
});

describe("A request", () => {
  const request = messageView({ itemId: iid(1), text: "Hi, I'm Kim", request: true });
  const asked = conversationView(KIM, [request]);

  async function inRequest(view: CommunityView = signedIn) {
    api.communityConversations.mockResolvedValue([{ ...KIM, unseen: 1 }]);
    api.communityConversation.mockResolvedValue(asked);
    const shown = await inMessages(view);
    await openRow(shown.user, "kim-ode", "Kim Ode");
    await screen.findByText("Hi, I'm Kim");
    return shown;
  }

  it("shows the first message with Accept and Leave this conversation, and no box to write in", async () => {
    await inRequest();
    expect(screen.getByRole("button", { name: "Accept" })).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Leave this conversation" })).toHaveLength(1);
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    // Nothing is answered before you accept.
    expect(screen.queryByRole("button", { name: "Reply" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "React" })).not.toBeInTheDocument();
  });

  it("accepts, and reads the conversation again", async () => {
    api.acceptCommunityRequest.mockResolvedValue(undefined);
    const { user } = await inRequest();
    await user.click(screen.getByRole("button", { name: "Accept" }));
    expect(api.acceptCommunityRequest).toHaveBeenCalledWith(mid(2));
    await waitFor(() => expect(api.communityConversation.mock.calls.length).toBeGreaterThan(1));
  });

  it("leaves after asking, with the words of ADR-173", async () => {
    api.leaveCommunityConversation.mockResolvedValue(undefined);
    const { user } = await inRequest();
    await user.click(screen.getByRole("button", { name: "Leave this conversation" }));
    const ask = await dialog("Leave this conversation?");
    expect(ask).toHaveTextContent(
      "It is deleted from this PC, and Kim Ode's new messages won't be delivered. Your other PCs keep their copies.",
    );
    expect(api.leaveCommunityConversation).not.toHaveBeenCalled();
    await user.click(within(ask).getByRole("button", { name: "Leave this conversation" }));
    expect(api.leaveCommunityConversation).toHaveBeenCalledWith(mid(2));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("says why when it can't accept", async () => {
    api.acceptCommunityRequest.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "8 West can't be reached right now."),
    );
    const { user } = await inRequest();
    await user.click(screen.getByRole("button", { name: "Accept" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "8 West can't be reached right now.",
    );
  });

  it("warns a member under 18 about someone they don't know, above the message", async () => {
    await inRequest(under18);
    const note = screen.getByRole("note");
    expect(note).toHaveTextContent(
      "You don't know this person yet. Never share passwords, keys, or where you live.",
    );
    expect(
      note.compareDocumentPosition(screen.getByText("Hi, I'm Kim")) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("does not warn an adult", async () => {
    await inRequest(signedIn);
    expect(screen.queryByText(/You don't know this person yet/)).not.toBeInTheDocument();
  });

  it("does not warn a member under 18 in a conversation that was accepted", async () => {
    await inPat(conversationView(PAT, [messageView()]), under18);
    expect(screen.queryByText(/You don't know this person yet/)).not.toBeInTheDocument();
  });

  it("groups it under Requests in the list", async () => {
    await inRequest();
    expect(
      within(screen.getByRole("list", { name: "Requests" })).getByRole("button", {
        name: /@kim-ode/,
      }),
    ).toBeInTheDocument();
  });
});

describe("The safety code", () => {
  it("shows the 12 digits in three groups, and what to do with them", async () => {
    const { user } = await inPat();
    expect(screen.queryByText("5373 9207 7552")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Check the safety code" }));
    expect(screen.getByText("5373 9207 7552")).toBeVisible();
    expect(
      screen.getByText(
        "Compare it with Pat Lee by phone or in person. If it matches, no one is in the middle.",
      ),
    ).toBeVisible();
  });

  it("is marked checked with It matches", async () => {
    api.communitySafetyCodeChecked.mockResolvedValue(undefined);
    const { user } = await inPat();
    await user.click(screen.getByRole("button", { name: "Check the safety code" }));
    expect(api.communitySafetyCodeChecked).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "It matches" }));
    expect(api.communitySafetyCodeChecked).toHaveBeenCalledWith(mid(1));
    expect(
      await screen.findByText(
        /Thank you\. Plenipo tells you if Pat Lee's computers change again\./,
      ),
    ).toBeVisible();
    expect(screen.queryByText("5373 9207 7552")).not.toBeInTheDocument();
  });

  it("says when there is no code yet, and offers no It matches", async () => {
    const { user } = await inPat(conversationView(PAT, [], null));
    await user.click(screen.getByRole("button", { name: "Check the safety code" }));
    expect(screen.getByText(/The safety code isn't ready yet/)).toBeVisible();
    expect(screen.queryByRole("button", { name: "It matches" })).not.toBeInTheDocument();
  });

  it("says when their computers changed, and to check it again", async () => {
    const changed = conversationSummary({ ...PAT, computersChanged: true });
    api.communityConversations.mockResolvedValue([changed]);
    await inPat(conversationView(changed, [messageView()]));
    const note = screen.getByRole("note");
    expect(note).toHaveTextContent("Pat Lee's computers changed. Check the safety code again.");
    expect(within(note).getByText("Pat Lee's computers changed").tagName).toBe("STRONG");
  });

  it("has no notice while nothing changed", async () => {
    await inPat();
    expect(screen.queryByRole("note")).not.toBeInTheDocument();
  });
});

describe("Where a conversation stands", () => {
  const stand = async (state: string) => {
    const person = conversationSummary({ ...PAT, state });
    api.communityConversations.mockResolvedValue([person]);
    return inPat(conversationView(person, state === "leftByMe" ? [] : [messageView()]));
  };

  it("waits for them to accept your first message, with no box to write in", async () => {
    await stand("requestedByMe");
    expect(screen.getByText("Waiting for @pat-lee to accept your first message.")).toBeVisible();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Reply" })).not.toBeInTheDocument();
    // You can take it back.
    expect(screen.getByRole("button", { name: "Leave this conversation" })).toBeInTheDocument();
  });

  it("says you left, and lets you write again", async () => {
    await stand("leftByMe");
    expect(
      screen.getByText("You left this conversation. Writing again opens it on your side."),
    ).toBeVisible();
    expect(box()).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Leave this conversation" }),
    ).not.toBeInTheDocument();
  });

  it("says they left, and that your messages won't be delivered", async () => {
    await stand("leftByThem");
    expect(
      screen.getByText("@pat-lee left this conversation. Your messages won't be delivered."),
    ).toBeVisible();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("has the box, and Leave this conversation, when it is accepted", async () => {
    await stand("accepted");
    expect(box()).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Leave this conversation" })).toBeInTheDocument();
    expect(screen.queryByText(/Waiting for/)).not.toBeInTheDocument();
  });

  it("leaves from the header, after asking, and shows what is left", async () => {
    api.leaveCommunityConversation.mockResolvedValue(undefined);
    const { user } = await stand("accepted");
    await user.click(screen.getByRole("button", { name: "Leave this conversation" }));
    const ask = await dialog("Leave this conversation?");
    expect(ask).toHaveTextContent(
      "It is deleted from this PC, and Pat Lee's new messages won't be delivered. Your other PCs keep their copies.",
    );
    // On a Mac, the words are a Mac's.
    await user.click(within(ask).getByRole("button", { name: "Cancel" }));
    setSystemWords(SYSTEM_WORDS.mac);
    await user.click(screen.getByRole("button", { name: "Leave this conversation" }));
    expect(await dialog("Leave this conversation?")).toHaveTextContent(
      "It is deleted from this Mac, and Pat Lee's new messages won't be delivered. Your other Macs keep their copies.",
    );
    expect(api.leaveCommunityConversation).not.toHaveBeenCalled();
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", { name: "Leave this conversation" }),
    );
    expect(api.leaveCommunityConversation).toHaveBeenCalledWith(mid(1));
  });

  it("says when it can't leave", async () => {
    api.leaveCommunityConversation.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "8 West can't be reached right now."),
    );
    const { user } = await stand("accepted");
    await user.click(screen.getByRole("button", { name: "Leave this conversation" }));
    const ask = await dialog("Leave this conversation?");
    await user.click(within(ask).getByRole("button", { name: "Leave this conversation" }));
    expect(await within(ask).findByRole("alert")).toHaveTextContent(
      "8 West can't be reached right now.",
    );
  });
});

describe("Starting a conversation", () => {
  const found = cardView({ memberId: mid(8), name: "pat-lee", displayName: "Pat Lee" });

  /** The Community section on People, with Pat's card found. */
  async function findPat() {
    api.getCommunity.mockResolvedValue(signedIn);
    api.findInCommunity.mockResolvedValue({ kind: "card", card: found });
    api.communityConversation.mockResolvedValue(null);
    const user = userEvent.setup();
    render(<PeoplePage go={go} />);
    await user.type(
      await screen.findByRole("textbox", { name: "Their name in Community" }),
      "pat-lee{Enter}",
    );
    const card = await screen.findByRole("article", { name: "@pat-lee" });
    return { user, card };
  }

  it("is a Message button on a card, which opens the Messages tab with a box for that person", async () => {
    const { user, card } = await findPat();
    await user.click(within(card).getByRole("button", { name: "Message" }));
    expect(screen.getByRole("tab", { name: /Messages/ })).toHaveAttribute("aria-selected", "true");
    expect(api.communityConversation).toHaveBeenCalledWith(mid(8), null);
    // There is no conversation yet: the box is there anyway, and says what a first message is.
    expect(await screen.findByRole("heading", { level: 2, name: "@pat-lee" })).toBeInTheDocument();
    expect(box("Message to @pat-lee")).toBeInTheDocument();
    expect(
      screen.getByText("Your first message is a request. @pat-lee can answer once they accept it."),
    ).toBeVisible();
  });

  it("sends the first message to that person, with their name", async () => {
    api.sendCommunityMessage.mockResolvedValue(
      messageView({ outgoing: true, state: "delivered", request: true }),
    );
    const { user, card } = await findPat();
    await user.click(within(card).getByRole("button", { name: "Message" }));
    await user.type(
      await screen.findByRole("textbox", { name: "Message to @pat-lee" }),
      "Hello Pat{Enter}",
    );
    expect(api.sendCommunityMessage).toHaveBeenCalledWith(mid(8), "pat-lee", "Hello Pat", null);
  });

  it("shows the words of Part of Pro when a Free copy starts a conversation", async () => {
    const words = "Starting a conversation, a link, or an invitation in Community is part of Pro.";
    api.sendCommunityMessage.mockRejectedValue(
      new commands.PlenipoCommandError("partOfPro", words),
    );
    const { user, card } = await findPat();
    await user.click(within(card).getByRole("button", { name: "Message" }));
    await user.type(
      await screen.findByRole("textbox", { name: "Message to @pat-lee" }),
      "Hello{Enter}",
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(words);
  });

  it("is on the cards in the directory too, and People stays as it was when you come back", async () => {
    api.getCommunity.mockResolvedValue(signedIn);
    api.communityDirectory.mockResolvedValue({
      cards: [found, cardView({ memberId: mid(9), name: "kim-ode", displayName: "Kim Ode" })],
      next: null,
    });
    api.communityConversation.mockResolvedValue(null);
    const user = userEvent.setup();
    render(<PeoplePage go={go} />);
    await user.click(await screen.findByRole("button", { name: "Search" }));
    const kim = await screen.findByRole("article", { name: "@kim-ode" });
    await user.click(within(kim).getByRole("button", { name: "Message" }));
    expect(await screen.findByRole("heading", { level: 2, name: "@kim-ode" })).toBeInTheDocument();
    expect(api.communityConversation).toHaveBeenCalledWith(mid(9), null);
    await user.click(screen.getByRole("tab", { name: "People" }));
    expect(screen.getByRole("article", { name: "@kim-ode" })).toBeInTheDocument();
    expect(screen.getByRole("article", { name: "@pat-lee" })).toBeInTheDocument();
  });

  it("is not on your own card", async () => {
    api.getCommunity.mockResolvedValue(signedIn);
    api.findInCommunity.mockResolvedValue({
      kind: "card",
      card: cardView({ memberId: mid(10), name: "frank-g", displayName: "Frank" }),
    });
    const user = userEvent.setup();
    render(<PeoplePage go={go} />);
    await user.type(
      await screen.findByRole("textbox", { name: "Their name in Community" }),
      "frank-g{Enter}",
    );
    const card = await screen.findByRole("article", { name: "@frank-g" });
    expect(within(card).queryByRole("button", { name: "Message" })).not.toBeInTheDocument();
  });

  it("is a Send a message request button for a name that can only be sent a request", async () => {
    api.getCommunity.mockResolvedValue(signedIn);
    api.findInCommunity.mockResolvedValue({
      kind: "requestOnly",
      memberId: mid(7),
      name: "kim-ode",
    });
    api.communityConversation.mockResolvedValue(null);
    const user = userEvent.setup();
    render(<PeoplePage go={go} />);
    await user.type(
      await screen.findByRole("textbox", { name: "Their name in Community" }),
      "kim-ode{Enter}",
    );
    expect(await screen.findByText("@kim-ode can only be sent a message request.")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Send a message request" }));
    expect(api.communityConversation).toHaveBeenCalledWith(mid(7), null);
    expect(await screen.findByRole("heading", { level: 2, name: "@kim-ode" })).toBeInTheDocument();
    expect(box("Message to @kim-ode")).toBeInTheDocument();
  });

  it("has no button for a name that is not found", async () => {
    api.getCommunity.mockResolvedValue(signedIn);
    api.findInCommunity.mockResolvedValue({ kind: "noOne" });
    const user = userEvent.setup();
    render(<PeoplePage go={go} />);
    await user.type(
      await screen.findByRole("textbox", { name: "Their name in Community" }),
      "nobody{Enter}",
    );
    await screen.findByText("No one in Community has that name.");
    expect(screen.queryByRole("button", { name: /Message/ })).not.toBeInTheDocument();
  });
});

describe("Messages arriving and changing", () => {
  /** What 8 West's Rust side does: reading a conversation tells every window. */
  function echoing(read: () => ReturnType<typeof conversationView>) {
    api.communityConversation.mockImplementation(() => {
      heard.current();
      return Promise.resolve(read());
    });
  }
  const later = (ms: number) => vi.spyOn(Date, "now").mockReturnValue(Date.now() + ms);
  const settle = () => act(() => new Promise<void>((done) => setTimeout(done, 40)));
  /** Messages changed: what the page hears from the app. */
  const fire = () =>
    act(async () => {
      heard.current();
      await Promise.resolve();
    });

  it("reads the list again when messages change, and the number on the tab moves", async () => {
    api.getCommunity.mockResolvedValue(signedIn);
    render(<PeoplePage go={go} />);
    const tab = await screen.findByRole("tab", { name: "Messages" });
    await waitFor(() => expect(api.communityConversations).toHaveBeenCalledTimes(1));
    api.communityConversations.mockResolvedValue([{ ...PAT, unseen: 4 }]);
    await fire();
    expect(await within(tab).findByRole("img", { name: "4 unseen" })).toBeInTheDocument();
    expect(api.communityConversation).not.toHaveBeenCalled();
  });

  it("does not read an open conversation again for the echo of reading it", async () => {
    echoing(() => conversationView(PAT, [messageView()]));
    const { user } = await inMessages();
    await openRow(user, "pat-lee", "Pat Lee");
    await settle();
    // Reading it told every window; hearing that must not start another read, or it never stops.
    expect(api.communityConversation).toHaveBeenCalledTimes(1);
    await fire();
    await settle();
    expect(api.communityConversation).toHaveBeenCalledTimes(1);
  });

  it("reads an open conversation again for a change that is not that echo", async () => {
    echoing(() => conversationView(PAT, [messageView()]));
    const { user } = await inMessages();
    await openRow(user, "pat-lee", "Pat Lee");
    await settle();
    expect(api.communityConversation).toHaveBeenCalledTimes(1);
    later(QUIET_MS + 1000);
    await fire();
    await waitFor(() => expect(api.communityConversation).toHaveBeenCalledTimes(2));
    // And the echo of that one does not start a third.
    await settle();
    expect(api.communityConversation).toHaveBeenCalledTimes(2);
  });

  it("shows a message that arrives right after a read, because the list says it is new", async () => {
    let messages = [messageView({ itemId: iid(1), text: "First" })];
    echoing(() => conversationView(PAT, messages));
    const { user } = await inMessages();
    await openRow(user, "pat-lee", "Pat Lee");
    await screen.findByText("First");
    await settle();
    messages = [
      ...messages,
      messageView({ itemId: iid(2), text: "Just arrived", acceptedAt: 1_790_000_999 }),
    ];
    api.communityConversations.mockResolvedValue([{ ...PAT, unseen: 1, lastAt: 1_790_000_999 }]);
    // Right away (inside the quiet time after the read): the list tells it.
    await fire();
    expect(await screen.findByText("Just arrived")).toBeVisible();
    await settle();
    // It is read again, but does not go on reading.
    expect(api.communityConversation.mock.calls.length).toBeLessThanOrEqual(4);
  });

  it("shows that they left when the list says the conversation changed", async () => {
    let state = "accepted";
    echoing(() => conversationView({ ...PAT, state }, [messageView()]));
    const { user } = await inMessages();
    await openRow(user, "pat-lee", "Pat Lee");
    await screen.findByText("Hello");
    await settle();
    state = "leftByThem";
    api.communityConversations.mockResolvedValue([{ ...PAT, state }]);
    await fire();
    expect(
      await screen.findByText("@pat-lee left this conversation. Your messages won't be delivered."),
    ).toBeVisible();
  });
});
