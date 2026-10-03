import { SYSTEM_WORDS, type CommunityView, type MessageView, type Stage } from "@plenipo/types";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { setSystemWords } from "../system/words";
import { a11yProblems } from "../test/a11y";
import {
  cardView,
  communityView,
  conversationSummary,
  conversationView,
  messageView,
} from "../test/communityFixtures";
import { CommunityCard } from "./CommunityCard";
import { CommunitySettings } from "./CommunitySettings";
import { MAX_NOTE_CHARS, MAX_REPORT_MESSAGES, REASONS } from "./blockReportWords";
import { onDate } from "./messageWords";
import { PeoplePage } from "./PeoplePage";
import { forgetPictures } from "./pictures";
import { ReportDialog, type ReportAbout } from "./ReportDialog";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getCommunity: vi.fn(),
    findInCommunity: vi.fn(),
    communityPicture: vi.fn(),
    communityConversations: vi.fn(),
    communityConversation: vi.fn(),
    sendCommunityMessage: vi.fn(),
    acceptCommunityRequest: vi.fn(),
    leaveCommunityConversation: vi.fn(),
    openCommunityLink: vi.fn(),
    setCommunitySwitch: vi.fn(),
    signOutOfCommunity: vi.fn(),
    blockInCommunity: vi.fn(),
    unblockInCommunity: vi.fn(),
    communityBlocked: vi.fn(),
    reportInCommunity: vi.fn(),
    deleteMyCommunityData: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeCommunity: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeCommunityMessages: vi.fn(() => Promise.resolve(() => undefined)),
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

const PAT = conversationSummary({ memberId: mid(1), name: "pat-lee", displayName: "Pat Lee" });
const KIM = conversationSummary({
  memberId: mid(2),
  name: "kim-ode",
  displayName: "Kim Ode",
  state: "requestedByThem",
});

/** Every character a person cannot see (or that can disguise words), as a screen must not show. */
// eslint-disable-next-line no-control-regex
const HIDDEN = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f\u200B-\u200F\u2028-\u202E\u2060-\u2069]/;

const THANKS = "Thanks. 8 West will look at this.";

beforeEach(() => {
  vi.clearAllMocks();
  for (const command of [
    api.getCommunity,
    api.findInCommunity,
    api.sendCommunityMessage,
    api.acceptCommunityRequest,
    api.leaveCommunityConversation,
    api.openCommunityLink,
    api.setCommunitySwitch,
    api.signOutOfCommunity,
    api.blockInCommunity,
    api.unblockInCommunity,
    api.communityBlocked,
    api.reportInCommunity,
    api.deleteMyCommunityData,
  ]) {
    command.mockReset();
  }
  forgetPictures();
  api.communityConversations.mockResolvedValue([PAT]);
  api.communityConversation.mockResolvedValue(conversationView(PAT, [messageView()]));
  api.communityBlocked.mockResolvedValue([]);
});

afterEach(() => {
  setSystemWords(SYSTEM_WORDS.windows);
});

const dialog = (name: string | RegExp) => screen.findByRole("dialog", { name });
const reasonBox = (label: string) => screen.getByRole("radio", { name: label });
const sendButton = () => screen.getByRole("button", { name: "Send report" });

/** `n` messages of Pat's, each with its own words. */
function manyMessages(n: number): MessageView[] {
  return Array.from({ length: n }, (_, i) =>
    messageView({ itemId: iid(i + 1), text: `Message number ${i + 1}`, sentAt: 1_790_000_000 + i }),
  );
}

describe("The Report window", () => {
  /** The window on its own, for a person called Pat. */
  function open(
    props: {
      about?: ReportAbout[];
      messages?: MessageView[];
      ticked?: string[];
      name?: string;
      onSent?: (blocked: boolean) => void;
      onClose?: () => void;
    } = {},
  ) {
    const onSent = props.onSent ?? vi.fn();
    const onClose = props.onClose ?? vi.fn();
    const user = userEvent.setup();
    const shown = render(
      <main>
        <h1>Community</h1>
        <ReportDialog
          memberId={mid(1)}
          name={props.name ?? "pat-lee"}
          about={props.about ?? ["person"]}
          messages={props.messages ?? []}
          ticked={props.ticked ?? []}
          onSent={onSent}
          onClose={onClose}
        />
      </main>,
    );
    return { user, onSent, onClose, ...shown };
  }

  it("asks What is wrong? with the ten reasons of ADR-167, in its words, and none chosen", () => {
    open();
    expect(screen.getByRole("dialog", { name: "Report @pat-lee" })).toBeInTheDocument();
    const group = screen.getByRole("group", { name: "What is wrong?" });
    const radios = within(group).getAllByRole("radio");
    expect(radios.map((r) => r.closest("label")?.textContent)).toEqual([
      "Spam",
      "Harassment or threats",
      "A scam",
      "Hate",
      "Sexual content",
      "Someone under 13",
      "A risk to a young person",
      "Pretending to be someone else",
      "Cheating for points",
      "Something else",
    ]);
    expect(radios.every((r) => !(r as HTMLInputElement).checked)).toBe(true);
    expect(REASONS.map((r) => r.value)).toEqual([
      "spam",
      "harassment",
      "scam",
      "hate",
      "sexual",
      "under13",
      "youngPersonRisk",
      "impersonation",
      "cheating",
      "other",
    ]);
  });

  it("explains once what a report carries", () => {
    open();
    expect(
      screen.getAllByText(
        "A report carries only what you tick, with proof it's real. 8 West reads only what you report.",
      ),
    ).toHaveLength(1);
  });

  it("has Also block off to begin with, and the note box with its label and a counter", () => {
    open();
    expect(screen.getByRole("checkbox", { name: "Also block @pat-lee" })).not.toBeChecked();
    const note = screen.getByRole("textbox", {
      name: "Anything else 8 West should know (optional)",
    });
    expect(note).toHaveValue("");
    expect(screen.getByText("0 of 1000")).toBeInTheDocument();
  });

  it("can't be sent until a reason is chosen", async () => {
    const { user } = open();
    expect(sendButton()).toBeDisabled();
    await user.click(reasonBox("A scam"));
    expect(sendButton()).toBeEnabled();
  });

  it.each(REASONS.map((r) => [r.label, r.value] as const))(
    "sends exactly the person, the reason (%s), and nothing else",
    async (label, value) => {
      api.reportInCommunity.mockResolvedValue(undefined);
      const { user } = open();
      await user.click(reasonBox(label));
      await user.click(sendButton());
      expect(api.reportInCommunity).toHaveBeenCalledExactlyOnceWith(
        mid(1),
        "pat-lee",
        { kind: "person" },
        value,
        "",
        false,
      );
    },
  );

  it("sends their profile, when that is what was chosen", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    const { user } = open({ about: ["person", "profile"] });
    const asked = screen.getByRole("group", { name: "What are you reporting?" });
    expect(
      within(asked)
        .getAllByRole("radio")
        .map((r) => r.closest("label")?.textContent),
    ).toEqual(["The person", "Their profile"]);
    // The person, until another is chosen.
    expect(within(asked).getByRole("radio", { name: "The person" })).toBeChecked();
    await user.click(within(asked).getByRole("radio", { name: "Their profile" }));
    await user.click(reasonBox("Pretending to be someone else"));
    await user.click(sendButton());
    expect(api.reportInCommunity).toHaveBeenCalledExactlyOnceWith(
      mid(1),
      "pat-lee",
      { kind: "profile" },
      "impersonation",
      "",
      false,
    );
  });

  it("does not ask what is reported when there is only one thing it can be", () => {
    open({ about: ["person"] });
    expect(screen.queryByRole("group", { name: "What are you reporting?" })).toBeNull();
    expect(screen.queryByRole("group", { name: "Which messages?" })).toBeNull();
  });

  it("sends the ticked messages, in the order they are shown, and the note", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    const messages = manyMessages(4);
    const { user } = open({ about: ["messages"], messages });
    await user.click(screen.getByRole("checkbox", { name: /Message number 3/ }));
    await user.click(screen.getByRole("checkbox", { name: /Message number 1/ }));
    expect(screen.getByText("2 of 20 ticked")).toBeInTheDocument();
    await user.click(reasonBox("Harassment or threats"));
    await user.type(
      screen.getByRole("textbox", { name: "Anything else 8 West should know (optional)" }),
      "  He keeps writing.  ",
    );
    await user.click(sendButton());
    expect(api.reportInCommunity).toHaveBeenCalledExactlyOnceWith(
      mid(1),
      "pat-lee",
      { kind: "messages", itemIds: [iid(1), iid(3)] },
      "harassment",
      // The spaces around the words are not sent.
      "He keeps writing.",
      false,
    );
  });

  it("can't be sent for messages until one is ticked, even with a reason", async () => {
    const { user } = open({ about: ["messages"], messages: manyMessages(2) });
    await user.click(reasonBox("Spam"));
    expect(sendButton()).toBeDisabled();
    await user.click(screen.getByRole("checkbox", { name: /Message number 2/ }));
    expect(sendButton()).toBeEnabled();
    await user.click(screen.getByRole("checkbox", { name: /Message number 2/ }));
    expect(sendButton()).toBeDisabled();
    expect(api.reportInCommunity).not.toHaveBeenCalled();
  });

  it("has the message it was opened from ticked, and no other", () => {
    open({ about: ["messages"], messages: manyMessages(3), ticked: [iid(2)] });
    expect(screen.getByRole("checkbox", { name: /Message number 1/ })).not.toBeChecked();
    expect(screen.getByRole("checkbox", { name: /Message number 2/ })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: /Message number 3/ })).not.toBeChecked();
    expect(screen.getByText("1 of 20 ticked")).toBeInTheDocument();
  });

  it("lets at most 20 messages be ticked", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    const { user } = open({ about: ["messages"], messages: manyMessages(25) });
    const boxes = screen
      .getAllByRole("checkbox")
      .filter((b) => b.closest(".report__message") !== null);
    expect(boxes).toHaveLength(25);
    for (const box of boxes) {
      if (!(box as HTMLInputElement).disabled) await user.click(box);
    }
    expect(boxes.filter((b) => (b as HTMLInputElement).checked)).toHaveLength(MAX_REPORT_MESSAGES);
    expect(MAX_REPORT_MESSAGES).toBe(20);
    // The ones left are off, and it says why.
    expect(boxes.filter((b) => (b as HTMLInputElement).disabled)).toHaveLength(5);
    expect(
      screen.getByText("20 of 20 ticked. That is the most one report can carry."),
    ).toBeInTheDocument();
    // Taking one back makes room for another.
    const first = boxes[0] as HTMLInputElement;
    await user.click(first);
    expect(first).not.toBeChecked();
    await user.click(boxes[24] as HTMLInputElement);
    expect(boxes[24]).toBeChecked();
    await user.click(reasonBox("Spam"));
    await user.click(sendButton());
    const call = api.reportInCommunity.mock.calls[0];
    const of = call?.[2];
    expect(of?.kind).toBe("messages");
    expect(of?.kind === "messages" ? of.itemIds : []).toHaveLength(20);
  });

  it("starts with at most 20 ticked, however many it was given", () => {
    const messages = manyMessages(25);
    open({ about: ["messages"], messages, ticked: messages.map((m) => m.itemId) });
    expect(
      screen.getByText("20 of 20 ticked. That is the most one report can carry."),
    ).toBeVisible();
  });

  it("says there is nothing to report, when there are no messages", async () => {
    const { user } = open({ about: ["messages"], messages: [] });
    expect(screen.getByText("There are no messages from @pat-lee here to report.")).toBeVisible();
    await user.click(reasonBox("Spam"));
    expect(sendButton()).toBeDisabled();
  });

  it("counts the note in characters, as the composer does, and stops at 1,000", () => {
    open();
    const note = screen.getByRole("textbox", {
      name: "Anything else 8 West should know (optional)",
    });
    expect(MAX_NOTE_CHARS).toBe(1000);
    fireEvent.change(note, { target: { value: "x".repeat(1005) } });
    expect(note).toHaveValue("x".repeat(1000));
    expect(screen.getByText("1000 of 1000")).toBeInTheDocument();
    // An emoji is one character, not two.
    fireEvent.change(note, { target: { value: "\u{1F600}".repeat(1000) } });
    expect(note).toHaveValue("\u{1F600}".repeat(1000));
    expect(screen.getByText("1000 of 1000")).toBeInTheDocument();
    // Too long is cut, never in the middle of an emoji.
    fireEvent.change(note, { target: { value: "\u{1F600}".repeat(1003) } });
    expect(note).toHaveValue("\u{1F600}".repeat(1000));
    fireEvent.change(note, { target: { value: "hello" } });
    expect(screen.getByText("5 of 1000")).toBeInTheDocument();
  });

  it("sends a note of exactly 1,000 characters whole", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    const { user } = open();
    fireEvent.change(
      screen.getByRole("textbox", { name: "Anything else 8 West should know (optional)" }),
      { target: { value: "y".repeat(1000) } },
    );
    await user.click(reasonBox("Something else"));
    await user.click(sendButton());
    expect(api.reportInCommunity).toHaveBeenCalledExactlyOnceWith(
      mid(1),
      "pat-lee",
      { kind: "person" },
      "other",
      "y".repeat(1000),
      false,
    );
  });

  it("says Thanks when it is sent, and then only offers Done", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    const { user, onSent, onClose } = open();
    await user.click(reasonBox("Hate"));
    await user.click(sendButton());
    expect(await screen.findByText(THANKS)).toBeInTheDocument();
    expect(screen.getByText(THANKS).tagName).toBe("STRONG");
    expect(onSent).toHaveBeenCalledExactlyOnceWith(false);
    expect(screen.queryByRole("button", { name: "Send report" })).toBeNull();
    expect(screen.queryByRole("radio")).toBeNull();
    // Not blocked: it does not say so.
    expect(screen.queryByText(/is blocked/)).toBeNull();
    await user.click(screen.getByRole("button", { name: "Done" }));
    expect(onClose).toHaveBeenCalledOnce();
    // Sending again is not possible: one press, one report.
    expect(api.reportInCommunity).toHaveBeenCalledOnce();
  });

  it("blocks too when Also block is ticked, and says so", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    const { user, onSent } = open();
    await user.click(reasonBox("A scam"));
    await user.click(screen.getByRole("checkbox", { name: "Also block @pat-lee" }));
    await user.click(sendButton());
    expect(api.reportInCommunity).toHaveBeenCalledExactlyOnceWith(
      mid(1),
      "pat-lee",
      { kind: "person" },
      "scam",
      "",
      true,
    );
    expect(await screen.findByText(THANKS)).toBeInTheDocument();
    expect(onSent).toHaveBeenCalledExactlyOnceWith(true);
    expect(
      screen.getByText("@pat-lee is blocked. You can undo it in Settings → Community → Blocked."),
    ).toBeInTheDocument();
  });

  it("does not block when nothing says to, and block is never a separate call", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    const { user } = open();
    await user.click(reasonBox("Spam"));
    await user.click(sendButton());
    await screen.findByText(THANKS);
    expect(api.blockInCommunity).not.toHaveBeenCalled();
  });

  it("says why when it could not be sent, and keeps what was typed", async () => {
    api.reportInCommunity.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "8 West can't be reached right now."),
    );
    const { user, onSent } = open({ about: ["messages"], messages: manyMessages(2) });
    await user.click(screen.getByRole("checkbox", { name: /Message number 2/ }));
    await user.click(reasonBox("Spam"));
    await user.click(sendButton());
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "8 West can't be reached right now.",
    );
    expect(screen.queryByText(THANKS)).toBeNull();
    expect(onSent).not.toHaveBeenCalled();
    expect(reasonBox("Spam")).toBeChecked();
    expect(screen.getByRole("checkbox", { name: /Message number 2/ })).toBeChecked();
    // It can be tried again.
    api.reportInCommunity.mockResolvedValue(undefined);
    await user.click(sendButton());
    expect(await screen.findByText(THANKS)).toBeInTheDocument();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("closes with Cancel and with Escape, and sends nothing", async () => {
    const { user, onClose } = open();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onClose).toHaveBeenCalledOnce();
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(2);
    expect(api.reportInCommunity).not.toHaveBeenCalled();
  });

  it("shows a link as plain text: no link, no button, nothing that opens", () => {
    open({
      about: ["messages"],
      messages: [
        messageView({ itemId: iid(1), text: "See https://example.com/a?b=1 for the plan." }),
      ],
    });
    const list = screen.getByRole("list");
    const address = within(list).getByText("https://example.com/a?b=1");
    expect(address.tagName).toBe("SPAN");
    expect(list.querySelector("a, [href], button")).toBeNull();
    expect(within(list).queryByRole("link")).toBeNull();
    expect(within(list).queryByRole("button", { name: "Open link" })).toBeNull();
    expect(list.textContent).toContain("See https://example.com/a?b=1 for the plan.");
    expect(api.openCommunityLink).not.toHaveBeenCalled();
  });

  it("shows hidden characters in a message as visible marks, and web code as text", () => {
    const words = "Send it to \u202Eevil\u200B now <img src=x onerror=alert(1)> <b>bold</b>";
    open({ about: ["messages"], messages: [messageView({ itemId: iid(1), text: words })] });
    const list = screen.getByRole("list");
    expect(list.textContent).not.toMatch(HIDDEN);
    expect([...list.querySelectorAll(".message__mark")].map((m) => m.textContent)).toEqual([
      "\u2039right-to-left override\u203A",
      "\u2039zero-width space\u203A",
    ]);
    expect(list.textContent).toContain("<img src=x onerror=alert(1)> <b>bold</b>");
    expect(list.querySelector("img, b, script, [onerror], a, [href]")).toBeNull();
  });

  it("says what a message with no words is, and shows its time", () => {
    open({
      about: ["messages"],
      messages: [
        messageView({ itemId: iid(1), text: null, hasSticker: true }),
        messageView({ itemId: iid(2), text: null, hasGif: true }),
        messageView({ itemId: iid(3), text: "", acceptedAt: 1_790_000_100 }),
      ],
    });
    const rows = within(screen.getByRole("list")).getAllByRole("listitem");
    expect(rows[0]).toHaveTextContent("A sticker");
    expect(rows[1]).toHaveTextContent("A GIF (this version doesn't show GIFs)");
    expect(rows[2]).toHaveTextContent("This message can't be shown in this version of Plenipo.");
    expect(rows[2]?.querySelector("time")).toHaveAttribute(
      "datetime",
      new Date(1_790_000_100 * 1000).toISOString(),
    );
  });

  it("shows a name with hidden characters or web code as text, and sends it as it came", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    const name = "pat\u202Elee<b>x</b>";
    const { user } = open({ name });
    const box = screen.getByRole("dialog");
    expect(box.textContent).not.toMatch(HIDDEN);
    expect(box.querySelector("b")).toBeNull();
    expect(
      screen.getByRole("dialog", { name: "Report @pat\uFFFDlee<b>x</b>" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", { name: "Also block @pat\uFFFDlee<b>x</b>" }),
    ).toBeInTheDocument();
    await user.click(reasonBox("Spam"));
    await user.click(sendButton());
    // The name that goes to 8 West is the one that came, not the one that is shown.
    expect(api.reportInCommunity).toHaveBeenCalledExactlyOnceWith(
      mid(1),
      name,
      { kind: "person" },
      "spam",
      "",
      false,
    );
    expect(await screen.findByText(THANKS)).toBeInTheDocument();
  });

  it("passes the accessibility smoke check, asking and thanking", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    const { user, container } = open({
      about: ["person", "messages"],
      messages: manyMessages(3),
      ticked: [iid(1)],
    });
    await user.click(screen.getByRole("radio", { name: "Messages from them" }));
    expect(a11yProblems(container)).toEqual([]);
    await user.click(reasonBox("Spam"));
    await user.click(sendButton());
    await screen.findByText(THANKS);
    expect(a11yProblems(container)).toEqual([]);
  });
});

/** The Community section on its Messages tab. */
async function inMessages(view: CommunityView = signedIn) {
  api.getCommunity.mockResolvedValue(view);
  const user = userEvent.setup();
  const shown = render(<PeoplePage go={go} />);
  await user.click(await screen.findByRole("tab", { name: /Messages/ }));
  return { user, ...shown };
}

/** Pat's conversation, open. */
async function inPat(conversation = conversationView(PAT, [messageView()])) {
  api.communityConversation.mockResolvedValue(conversation);
  const shown = await inMessages();
  await shown.user.click(await screen.findByRole("button", { name: /@pat-lee/ }));
  await screen.findByRole("heading", { level: 2, name: "Pat Lee" });
  await screen.findByText(/^Sealed: only you and /);
  return shown;
}

const headerOf = () =>
  within(
    screen
      .getByRole("heading", { level: 2, name: /^(Pat Lee|Kim Ode)$/ })
      .closest("header") as HTMLElement,
  );
const items = () => within(screen.getByRole("list", { name: "Messages" })).getAllByRole("listitem");
const box = () => screen.getByRole("textbox", { name: "Message to Pat Lee" });

describe("Report, in a conversation", () => {
  it("is in the header, for the person, with Block beside it", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    const { user } = await inPat();
    expect(headerOf().getByRole("button", { name: "Block" })).toBeInTheDocument();
    await user.click(headerOf().getByRole("button", { name: "Report" }));
    const ask = await dialog("Report @pat-lee");
    // The person: no list of messages, and no choice to make.
    expect(within(ask).queryByRole("group", { name: "Which messages?" })).toBeNull();
    expect(within(ask).queryByRole("group", { name: "What are you reporting?" })).toBeNull();
    await user.click(within(ask).getByRole("radio", { name: "Spam" }));
    await user.click(within(ask).getByRole("button", { name: "Send report" }));
    expect(api.reportInCommunity).toHaveBeenCalledExactlyOnceWith(
      mid(1),
      "pat-lee",
      { kind: "person" },
      "spam",
      "",
      false,
    );
    expect(await within(ask).findByText(THANKS)).toBeInTheDocument();
    await user.click(within(ask).getByRole("button", { name: "Done" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    // Not blocked: the box to write in is still there.
    expect(box()).toBeInTheDocument();
    expect(screen.queryByText("You blocked @pat-lee.")).toBeNull();
  });

  it("is on each message of theirs, and opens with that message ticked", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    const { user } = await inPat(
      conversationView(PAT, [
        messageView({ itemId: iid(1), text: "First one" }),
        messageView({ itemId: iid(2), text: "Second one" }),
        messageView({
          itemId: iid(3),
          text: "Mine",
          outgoing: true,
          state: "delivered",
          reportable: false,
        }),
      ]),
    );
    const [first, second, mine] = items() as [HTMLElement, HTMLElement, HTMLElement];
    expect(within(first).getByRole("button", { name: "Report" })).toBeInTheDocument();
    expect(within(second).getByRole("button", { name: "Report" })).toBeInTheDocument();
    // Your own words are not reported.
    expect(within(mine).queryByRole("button", { name: "Report" })).toBeNull();
    // It comes after the other buttons in the row.
    expect(
      within(second)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["React", "Reply", "Delete for me", "Give to a worker", "Report"]);

    await user.click(within(second).getByRole("button", { name: "Report" }));
    const ask = await dialog("Report @pat-lee");
    const list = within(ask).getByRole("list");
    // Only their messages can be picked, and the one it was opened from is ticked.
    expect(within(list).getAllByRole("checkbox")).toHaveLength(2);
    expect(within(list).getByRole("checkbox", { name: /First one/ })).not.toBeChecked();
    expect(within(list).getByRole("checkbox", { name: /Second one/ })).toBeChecked();
    await user.click(within(ask).getByRole("radio", { name: "A scam" }));
    await user.click(within(ask).getByRole("button", { name: "Send report" }));
    expect(api.reportInCommunity).toHaveBeenCalledExactlyOnceWith(
      mid(1),
      "pat-lee",
      { kind: "messages", itemIds: [iid(2)] },
      "scam",
      "",
      false,
    );
    expect(await within(ask).findByText(THANKS)).toBeInTheDocument();
  });

  it("is not on a message that can't be reported (no proof)", async () => {
    await inPat(conversationView(PAT, [messageView({ text: "Old", reportable: false })]));
    expect(within(items()[0] as HTMLElement).queryByRole("button", { name: "Report" })).toBeNull();
    // The person can still be reported.
    expect(headerOf().getByRole("button", { name: "Report" })).toBeInTheDocument();
  });

  it("is on a message even when you can't write back (a request)", async () => {
    api.communityConversations.mockResolvedValue([{ ...KIM, unseen: 1 }]);
    api.communityConversation.mockResolvedValue(
      conversationView(KIM, [messageView({ text: "Hi, I'm Kim", request: true })]),
    );
    const { user } = await inMessages();
    await user.click(await screen.findByRole("button", { name: /@kim-ode/ }));
    await screen.findByText("Hi, I'm Kim");
    expect(within(items()[0] as HTMLElement).getByRole("button", { name: "Report" })).toBeVisible();
  });

  it("blocks too when Also block is ticked, and then says so, with Unblock", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    api.unblockInCommunity.mockResolvedValue(undefined);
    const { user } = await inPat();
    await user.click(headerOf().getByRole("button", { name: "Report" }));
    const ask = await dialog("Report @pat-lee");
    await user.click(within(ask).getByRole("radio", { name: "Harassment or threats" }));
    await user.click(within(ask).getByRole("checkbox", { name: "Also block @pat-lee" }));
    await user.click(within(ask).getByRole("button", { name: "Send report" }));
    expect(api.reportInCommunity).toHaveBeenCalledExactlyOnceWith(
      mid(1),
      "pat-lee",
      { kind: "person" },
      "harassment",
      "",
      true,
    );
    await within(ask).findByText(THANKS);
    await user.click(within(ask).getByRole("button", { name: "Done" }));
    expect(screen.getByText("You blocked @pat-lee.")).toBeVisible();
    // The block was made by the report: it is not made twice.
    expect(api.blockInCommunity).not.toHaveBeenCalled();
    expect(screen.queryByRole("textbox", { name: "Message to Pat Lee" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Unblock" }));
    expect(api.unblockInCommunity).toHaveBeenCalledExactlyOnceWith(mid(1));
    await waitFor(() => expect(screen.queryByText("You blocked @pat-lee.")).toBeNull());
    expect(box()).toBeInTheDocument();
  });
});

describe("Block, in a conversation", () => {
  it("asks first, in plain words, and calls only after you say yes", async () => {
    api.blockInCommunity.mockResolvedValue(undefined);
    const { user } = await inPat();
    await user.click(headerOf().getByRole("button", { name: "Block" }));
    const ask = await dialog("Block @pat-lee?");
    expect(ask).toHaveTextContent(
      "They can't message you, find your card, or link with you. They aren't told. What is on this PC stays.",
    );
    expect(api.blockInCommunity).not.toHaveBeenCalled();
    // No: nothing is blocked.
    await user.click(within(ask).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.blockInCommunity).not.toHaveBeenCalled();
    expect(box()).toBeInTheDocument();
    // Yes.
    await user.click(headerOf().getByRole("button", { name: "Block" }));
    await user.click(
      within(await dialog("Block @pat-lee?")).getByRole("button", { name: "Block" }),
    );
    expect(api.blockInCommunity).toHaveBeenCalledExactlyOnceWith(mid(1), "pat-lee");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("says You blocked @pat, with Unblock, and takes the box to write in away", async () => {
    api.blockInCommunity.mockResolvedValue(undefined);
    const { user } = await inPat();
    await user.click(headerOf().getByRole("button", { name: "Block" }));
    await user.click(
      within(await dialog("Block @pat-lee?")).getByRole("button", { name: "Block" }),
    );
    expect(await screen.findByText("You blocked @pat-lee.")).toBeVisible();
    expect(screen.getByRole("button", { name: "Unblock" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Message to Pat Lee" })).toBeNull();
    // It is done: no second Block, and nothing to react to or answer.
    expect(headerOf().queryByRole("button", { name: "Block" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Reply" })).toBeNull();
    expect(screen.queryByRole("button", { name: "React" })).toBeNull();
    // The person can still be reported.
    expect(headerOf().getByRole("button", { name: "Report" })).toBeInTheDocument();
    // The conversation was read again: the app knows it changed.
    await waitFor(() => expect(api.communityConversation.mock.calls.length).toBeGreaterThan(1));
  });

  it("keeps saying so when you look at another conversation and come back", async () => {
    api.blockInCommunity.mockResolvedValue(undefined);
    api.communityConversations.mockResolvedValue([PAT, { ...KIM, state: "accepted" }]);
    const { user } = await inPat();
    await user.click(headerOf().getByRole("button", { name: "Block" }));
    await user.click(
      within(await dialog("Block @pat-lee?")).getByRole("button", { name: "Block" }),
    );
    await screen.findByText("You blocked @pat-lee.");
    api.communityConversation.mockResolvedValue(
      conversationView({ ...KIM, state: "accepted" }, [messageView({ text: "Kim here" })]),
    );
    await user.click(screen.getByRole("button", { name: /@kim-ode/ }));
    await screen.findByText("Kim here");
    expect(screen.queryByText("You blocked @pat-lee.")).toBeNull();
    expect(screen.queryByText("You blocked @kim-ode.")).toBeNull();
    api.communityConversation.mockResolvedValue(conversationView(PAT, [messageView()]));
    await user.click(screen.getByRole("button", { name: /@pat-lee/ }));
    expect(await screen.findByText("You blocked @pat-lee.")).toBeVisible();
  });

  it("unblocks, and the box to write in comes back", async () => {
    api.blockInCommunity.mockResolvedValue(undefined);
    api.unblockInCommunity.mockResolvedValue(undefined);
    const { user } = await inPat();
    await user.click(headerOf().getByRole("button", { name: "Block" }));
    await user.click(
      within(await dialog("Block @pat-lee?")).getByRole("button", { name: "Block" }),
    );
    await user.click(await screen.findByRole("button", { name: "Unblock" }));
    expect(api.unblockInCommunity).toHaveBeenCalledExactlyOnceWith(mid(1));
    await waitFor(() => expect(screen.queryByText("You blocked @pat-lee.")).toBeNull());
    expect(box()).toBeInTheDocument();
    expect(headerOf().getByRole("button", { name: "Block" })).toBeInTheDocument();
  });

  it("says why when it can't block, and stays asking", async () => {
    api.blockInCommunity.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "8 West can't be reached right now."),
    );
    const { user } = await inPat();
    await user.click(headerOf().getByRole("button", { name: "Block" }));
    const ask = await dialog("Block @pat-lee?");
    await user.click(within(ask).getByRole("button", { name: "Block" }));
    expect(await within(ask).findByRole("alert")).toHaveTextContent(
      "8 West can't be reached right now.",
    );
    expect(screen.queryByText("You blocked @pat-lee.")).toBeNull();
    expect(box()).toBeInTheDocument();
  });

  it("says why when it can't unblock, and says still blocked", async () => {
    api.blockInCommunity.mockResolvedValue(undefined);
    api.unblockInCommunity.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "8 West can't be reached right now."),
    );
    const { user } = await inPat();
    await user.click(headerOf().getByRole("button", { name: "Block" }));
    await user.click(
      within(await dialog("Block @pat-lee?")).getByRole("button", { name: "Block" }),
    );
    await user.click(await screen.findByRole("button", { name: "Unblock" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "8 West can't be reached right now.",
    );
    expect(screen.getByText("You blocked @pat-lee.")).toBeVisible();
  });

  it("says this Mac, on a Mac", async () => {
    setSystemWords(SYSTEM_WORDS.mac);
    const { user } = await inPat();
    await user.click(headerOf().getByRole("button", { name: "Block" }));
    expect(await dialog("Block @pat-lee?")).toHaveTextContent("What is on this Mac stays.");
  });

  it("shows a name with hidden characters as text, and blocks the name as it came", async () => {
    api.blockInCommunity.mockResolvedValue(undefined);
    const odd = conversationSummary({ memberId: mid(9), name: "pat\u202Elee", displayName: null });
    api.communityConversations.mockResolvedValue([odd]);
    api.communityConversation.mockResolvedValue(conversationView(odd, [messageView()]));
    const { user } = await inMessages();
    await user.click(await screen.findByRole("button", { name: /@pat/ }));
    await screen.findByText(/^Sealed: only you and /);
    await user.click(screen.getByRole("button", { name: "Block" }));
    const ask = await dialog("Block @pat\uFFFDlee?");
    expect(ask.textContent).not.toMatch(HIDDEN);
    await user.click(within(ask).getByRole("button", { name: "Block" }));
    expect(api.blockInCommunity).toHaveBeenCalledExactlyOnceWith(mid(9), "pat\u202Elee");
    expect(await screen.findByText("You blocked @pat\uFFFDlee.")).toBeVisible();
  });
});

describe("A request", () => {
  async function inRequest() {
    api.communityConversations.mockResolvedValue([{ ...KIM, unseen: 1 }]);
    api.communityConversation.mockResolvedValue(
      conversationView(KIM, [messageView({ itemId: iid(1), text: "Hi, I'm Kim", request: true })]),
    );
    const shown = await inMessages();
    await shown.user.click(await screen.findByRole("button", { name: /@kim-ode/ }));
    await screen.findByText("Hi, I'm Kim");
    return shown;
  }

  it("has Accept, Block, Report, and Leave this conversation, each once", async () => {
    await inRequest();
    for (const name of ["Accept", "Block", "Report", "Leave this conversation"]) {
      expect(screen.getAllByRole("button", { name })).toHaveLength(
        // Their message has its own Report.
        name === "Report" ? 2 : 1,
      );
    }
    const footer = screen.getByRole("button", { name: "Accept" }).parentElement as HTMLElement;
    expect(
      within(footer)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Accept", "Block", "Report", "Leave this conversation"]);
    // The header has none of them (they are below, with Accept).
    expect(headerOf().queryByRole("button", { name: "Block" })).toBeNull();
    expect(headerOf().queryByRole("button", { name: "Report" })).toBeNull();
  });

  it("blocks, after asking, and then says so instead of offering Accept", async () => {
    api.blockInCommunity.mockResolvedValue(undefined);
    const { user } = await inRequest();
    const footer = screen.getByRole("button", { name: "Accept" }).parentElement as HTMLElement;
    await user.click(within(footer).getByRole("button", { name: "Block" }));
    const ask = await dialog("Block @kim-ode?");
    await user.click(within(ask).getByRole("button", { name: "Block" }));
    expect(api.blockInCommunity).toHaveBeenCalledExactlyOnceWith(mid(2), "kim-ode");
    expect(await screen.findByText("You blocked @kim-ode.")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Accept" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Leave this conversation" })).toBeNull();
    expect(screen.getByRole("button", { name: "Unblock" })).toBeInTheDocument();
  });

  it("reports the person", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    const { user } = await inRequest();
    const footer = screen.getByRole("button", { name: "Accept" }).parentElement as HTMLElement;
    await user.click(within(footer).getByRole("button", { name: "Report" }));
    const ask = await dialog("Report @kim-ode");
    await user.click(within(ask).getByRole("radio", { name: "A scam" }));
    await user.click(within(ask).getByRole("button", { name: "Send report" }));
    expect(api.reportInCommunity).toHaveBeenCalledExactlyOnceWith(
      mid(2),
      "kim-ode",
      { kind: "person" },
      "scam",
      "",
      false,
    );
  });
});

describe("Leave this conversation, Report first", () => {
  /** Asks to leave Pat's conversation. */
  async function asking(messages: MessageView[]) {
    api.leaveCommunityConversation.mockResolvedValue(undefined);
    const shown = await inPat(conversationView(PAT, messages));
    await shown.user.click(headerOf().getByRole("button", { name: "Leave this conversation" }));
    const ask = await dialog("Leave this conversation?");
    return { ...shown, ask };
  }

  it("is offered beside the question, and leaves nothing done by itself", async () => {
    const { ask } = await asking([messageView()]);
    expect(
      within(ask)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["×", "Report first", "Cancel", "Leave this conversation"]);
    expect(api.leaveCommunityConversation).not.toHaveBeenCalled();
  });

  it("opens the Report window for their messages, with none ticked, when there are some", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    const { user, ask } = await asking([
      messageView({ itemId: iid(1), text: "First one" }),
      messageView({ itemId: iid(2), text: "Second one" }),
    ]);
    await user.click(within(ask).getByRole("button", { name: "Report first" }));
    const report = await dialog("Report @pat-lee");
    // The question has gone: one window at a time.
    expect(screen.queryByRole("dialog", { name: "Leave this conversation?" })).toBeNull();
    expect(within(report).getByRole("group", { name: "Which messages?" })).toBeInTheDocument();
    expect(within(report).getAllByRole("checkbox", { name: /one/ })).toHaveLength(2);
    expect(within(report).getByRole("checkbox", { name: /First one/ })).not.toBeChecked();
    expect(within(report).getByRole("checkbox", { name: /Second one/ })).not.toBeChecked();
    // Nothing can be sent until a message is ticked.
    await user.click(within(report).getByRole("radio", { name: "Hate" }));
    expect(within(report).getByRole("button", { name: "Send report" })).toBeDisabled();
    await user.click(within(report).getByRole("checkbox", { name: /First one/ }));
    await user.click(within(report).getByRole("button", { name: "Send report" }));
    expect(api.reportInCommunity).toHaveBeenCalledExactlyOnceWith(
      mid(1),
      "pat-lee",
      { kind: "messages", itemIds: [iid(1)] },
      "hate",
      "",
      false,
    );
    await within(report).findByText(THANKS);
    // Reporting does not leave.
    expect(api.leaveCommunityConversation).not.toHaveBeenCalled();
  });

  it("opens the Report window for the person when there are no messages to report", async () => {
    const { user, ask } = await asking([messageView({ text: "Old", reportable: false })]);
    await user.click(within(ask).getByRole("button", { name: "Report first" }));
    const report = await dialog("Report @pat-lee");
    expect(within(report).queryByRole("group", { name: "Which messages?" })).toBeNull();
    expect(within(report).getByRole("group", { name: "What is wrong?" })).toBeInTheDocument();
  });

  it("goes back to the question when the Report window closes, and leaving still works", async () => {
    const { user, ask } = await asking([messageView()]);
    await user.click(within(ask).getByRole("button", { name: "Report first" }));
    const report = await dialog("Report @pat-lee");
    await user.click(within(report).getByRole("button", { name: "Cancel" }));
    const again = await dialog("Leave this conversation?");
    expect(api.reportInCommunity).not.toHaveBeenCalled();
    expect(api.leaveCommunityConversation).not.toHaveBeenCalled();
    await user.click(within(again).getByRole("button", { name: "Leave this conversation" }));
    expect(api.leaveCommunityConversation).toHaveBeenCalledExactlyOnceWith(mid(1));
  });
});

describe("Report and Block, on a card", () => {
  async function foundPat() {
    api.communityConversations.mockResolvedValue([]);
    api.findInCommunity.mockResolvedValue({
      kind: "card",
      card: cardView({ memberId: mid(1), name: "pat-lee", displayName: "Pat Lee" }),
    });
    api.getCommunity.mockResolvedValue(signedIn);
    const user = userEvent.setup();
    const shown = render(<PeoplePage go={go} />);
    await user.type(
      await screen.findByRole("textbox", { name: "Their name in Community" }),
      "pat-lee{Enter}",
    );
    const card = await screen.findByRole("article", { name: "@pat-lee" });
    return { user, card, ...shown };
  }

  it("is on a card, with Message and Block", async () => {
    const { card } = await foundPat();
    expect(
      within(card)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Message", "Report", "Block"]);
  });

  it("is not on your own card", async () => {
    api.communityConversations.mockResolvedValue([]);
    api.findInCommunity.mockResolvedValue({
      kind: "card",
      card: cardView({ memberId: mid(5), name: "frank-g", displayName: "Frank" }),
    });
    api.getCommunity.mockResolvedValue(signedIn);
    const user = userEvent.setup();
    render(<PeoplePage go={go} />);
    await user.type(
      await screen.findByRole("textbox", { name: "Their name in Community" }),
      "frank-g{Enter}",
    );
    const card = await screen.findByRole("article", { name: "@frank-g" });
    expect(within(card).queryAllByRole("button")).toHaveLength(0);
  });

  it("has no buttons at all on a card on its own", () => {
    render(<CommunityCard card={cardView()} />);
    expect(within(screen.getByRole("article")).queryAllByRole("button")).toHaveLength(0);
  });

  it("has only what it is given: Report and Block, with no Message", () => {
    const report = vi.fn();
    const block = vi.fn();
    render(<CommunityCard card={cardView()} onReport={report} onBlock={block} />);
    expect(
      within(screen.getByRole("article"))
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Report", "Block"]);
  });

  it("asks the person's ID and their name as it came, when Report or Block is pressed", async () => {
    const report = vi.fn();
    const block = vi.fn();
    const user = userEvent.setup();
    render(
      <CommunityCard
        card={cardView({ memberId: mid(3), name: "pat\u202Elee" })}
        onReport={report}
        onBlock={block}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Report" }));
    await user.click(screen.getByRole("button", { name: "Block" }));
    expect(report).toHaveBeenCalledExactlyOnceWith(mid(3), "pat\u202Elee");
    expect(block).toHaveBeenCalledExactlyOnceWith(mid(3), "pat\u202Elee");
  });

  it("reports the person or their profile: you choose", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    const { user, card } = await foundPat();
    await user.click(within(card).getByRole("button", { name: "Report" }));
    const ask = await dialog("Report @pat-lee");
    const choice = within(ask).getByRole("group", { name: "What are you reporting?" });
    expect(
      within(choice)
        .getAllByRole("radio")
        .map((r) => r.closest("label")?.textContent),
    ).toEqual(["The person", "Their profile"]);
    // No messages to tick on a card.
    expect(within(ask).queryByRole("group", { name: "Which messages?" })).toBeNull();
    await user.click(within(choice).getByRole("radio", { name: "Their profile" }));
    await user.click(within(ask).getByRole("radio", { name: "Pretending to be someone else" }));
    await user.click(within(ask).getByRole("button", { name: "Send report" }));
    expect(api.reportInCommunity).toHaveBeenCalledExactlyOnceWith(
      mid(1),
      "pat-lee",
      { kind: "profile" },
      "impersonation",
      "",
      false,
    );
    expect(await within(ask).findByText(THANKS)).toBeInTheDocument();
    await user.click(within(ask).getByRole("button", { name: "Done" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("says a person was blocked when the report blocked them", async () => {
    api.reportInCommunity.mockResolvedValue(undefined);
    const { user, card } = await foundPat();
    await user.click(within(card).getByRole("button", { name: "Report" }));
    const ask = await dialog("Report @pat-lee");
    await user.click(within(ask).getByRole("radio", { name: "Spam" }));
    await user.click(within(ask).getByRole("checkbox", { name: "Also block @pat-lee" }));
    await user.click(within(ask).getByRole("button", { name: "Send report" }));
    await within(ask).findByText(THANKS);
    expect(api.reportInCommunity).toHaveBeenCalledExactlyOnceWith(
      mid(1),
      "pat-lee",
      { kind: "person" },
      "spam",
      "",
      true,
    );
    await user.click(within(ask).getByRole("button", { name: "Done" }));
    expect(
      screen.getByText("You blocked @pat-lee. You can undo it in Settings → Community → Blocked."),
    ).toBeVisible();
  });

  it("blocks after asking, and says so", async () => {
    api.blockInCommunity.mockResolvedValue(undefined);
    const { user, card } = await foundPat();
    await user.click(within(card).getByRole("button", { name: "Block" }));
    const ask = await dialog("Block @pat-lee?");
    expect(ask).toHaveTextContent(
      "They can't message you, find your card, or link with you. They aren't told. What is on this PC stays.",
    );
    expect(api.blockInCommunity).not.toHaveBeenCalled();
    await user.click(within(ask).getByRole("button", { name: "Cancel" }));
    expect(api.blockInCommunity).not.toHaveBeenCalled();
    await user.click(within(card).getByRole("button", { name: "Block" }));
    await user.click(
      within(await dialog("Block @pat-lee?")).getByRole("button", { name: "Block" }),
    );
    expect(api.blockInCommunity).toHaveBeenCalledExactlyOnceWith(mid(1), "pat-lee");
    expect(
      await screen.findByText(
        "You blocked @pat-lee. You can undo it in Settings → Community → Blocked.",
      ),
    ).toBeVisible();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("says why when it can't block", async () => {
    api.blockInCommunity.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "8 West can't be reached right now."),
    );
    const { user, card } = await foundPat();
    await user.click(within(card).getByRole("button", { name: "Block" }));
    const ask = await dialog("Block @pat-lee?");
    await user.click(within(ask).getByRole("button", { name: "Block" }));
    expect(await within(ask).findByRole("alert")).toHaveTextContent(
      "8 West can't be reached right now.",
    );
  });
});

/** Settings → Community, in the stage a test names. */
function inSettings(patch: Partial<CommunityView> = {}) {
  api.getCommunity.mockResolvedValue({ ...signedIn, ...patch });
  const user = userEvent.setup();
  const shown = render(
    <main>
      <h1>Settings</h1>
      <h2>Community</h2>
      <CommunitySettings go={go} />
    </main>,
  );
  return { user, ...shown };
}

describe("Settings → Community → Blocked", () => {
  const PAT_BLOCKED = { memberId: mid(1), name: "pat-lee", blockedAt: 1_790_000_000 };
  const KIM_BLOCKED = { memberId: mid(2), name: "kim-ode", blockedAt: 1_790_100_000 };

  it("says No one is blocked, when no one is", async () => {
    inSettings();
    expect(await screen.findByRole("heading", { level: 3, name: "Blocked" })).toBeInTheDocument();
    expect(await screen.findByText("No one is blocked.")).toBeVisible();
    expect(screen.queryByRole("list", { name: "People you blocked" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Unblock" })).toBeNull();
  });

  it("is read when the section shows, once", async () => {
    inSettings();
    await screen.findByText("No one is blocked.");
    expect(api.communityBlocked).toHaveBeenCalledOnce();
  });

  it("lists each person with the date, in local time, and Unblock", async () => {
    api.communityBlocked.mockResolvedValue([PAT_BLOCKED, KIM_BLOCKED]);
    inSettings();
    const list = await screen.findByRole("list", { name: "People you blocked" });
    const rows = within(list).getAllByRole("listitem");
    expect(rows).toHaveLength(2);
    expect(within(rows[0] as HTMLElement).getByText("@pat-lee")).toBeInTheDocument();
    expect(rows[0]).toHaveTextContent(`Blocked on ${onDate(1_790_000_000)}`);
    expect(within(rows[1] as HTMLElement).getByText("@kim-ode")).toBeInTheDocument();
    expect(rows[1]).toHaveTextContent(`Blocked on ${onDate(1_790_100_000)}`);
    // Each button is "Unblock"; the name beside it says who.
    const buttons = within(list).getAllByRole("button", { name: "Unblock" });
    expect(buttons).toHaveLength(2);
    expect(buttons[0]).toHaveAccessibleDescription("@pat-lee");
    expect(buttons[1]).toHaveAccessibleDescription("@kim-ode");
    expect(screen.queryByText("No one is blocked.")).toBeNull();
  });

  it("unblocks one person, and the rest stay", async () => {
    api.communityBlocked.mockResolvedValue([PAT_BLOCKED, KIM_BLOCKED]);
    api.unblockInCommunity.mockResolvedValue(undefined);
    const { user } = inSettings();
    const list = await screen.findByRole("list", { name: "People you blocked" });
    const [, second] = within(list).getAllByRole("button", { name: "Unblock" });
    await user.click(second as HTMLElement);
    expect(api.unblockInCommunity).toHaveBeenCalledExactlyOnceWith(mid(2));
    await waitFor(() => expect(screen.queryByText("@kim-ode")).toBeNull());
    expect(screen.getByText("@pat-lee")).toBeInTheDocument();
    expect(
      screen.getByText("Unblocked @kim-ode. Links the block ended don't come back by themselves."),
    ).toBeVisible();
  });

  it("says No one is blocked after the last one is unblocked", async () => {
    api.communityBlocked.mockResolvedValue([PAT_BLOCKED]);
    api.unblockInCommunity.mockResolvedValue(undefined);
    const { user } = inSettings();
    await user.click(await screen.findByRole("button", { name: "Unblock" }));
    expect(await screen.findByText("No one is blocked.")).toBeVisible();
    expect(api.unblockInCommunity).toHaveBeenCalledExactlyOnceWith(mid(1));
  });

  it("says why when it can't unblock, and keeps the person in the list", async () => {
    api.communityBlocked.mockResolvedValue([PAT_BLOCKED]);
    api.unblockInCommunity.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "8 West can't be reached right now."),
    );
    const { user } = inSettings();
    await user.click(await screen.findByRole("button", { name: "Unblock" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "8 West can't be reached right now.",
    );
    expect(screen.getByText("@pat-lee")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Unblock" })).toBeEnabled();
  });

  it("says why when the list can't be read, and can try again", async () => {
    api.communityBlocked.mockRejectedValueOnce(
      new commands.PlenipoCommandError("invalidInput", "Plenipo is still starting."),
    );
    api.communityBlocked.mockResolvedValue([PAT_BLOCKED]);
    const { user } = inSettings();
    expect(await screen.findByRole("alert")).toHaveTextContent("Plenipo is still starting.");
    expect(screen.queryByText("No one is blocked.")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("@pat-lee")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows names with hidden characters or web code as text", async () => {
    api.communityBlocked.mockResolvedValue([
      { memberId: mid(1), name: "pat\u202Elee", blockedAt: 1_790_000_000 },
      { memberId: mid(2), name: "<img src=x onerror=alert(1)>", blockedAt: 1_790_000_000 },
    ]);
    api.unblockInCommunity.mockResolvedValue(undefined);
    const { user } = inSettings();
    const list = await screen.findByRole("list", { name: "People you blocked" });
    expect(list.textContent).not.toMatch(HIDDEN);
    expect(list.textContent).toContain("@pat\uFFFDlee");
    expect(within(list).getByText("@<img src=x onerror=alert(1)>")).toBeInTheDocument();
    expect(list.querySelector("img, [onerror], a, [href]")).toBeNull();
    // Unblock sends the person's ID, never the name.
    await user.click(within(list).getAllByRole("button", { name: "Unblock" })[0] as HTMLElement);
    expect(api.unblockInCommunity).toHaveBeenCalledExactlyOnceWith(mid(1));
  });

  it.each<[Stage, Partial<CommunityView>]>([
    ["off", {}],
    ["comingSoon", { comingSoon: true }],
    ["signedOut", {}],
    ["signingIn", { code: "4KQ-7TD" }],
    ["joining", { terms: "2026-10-01" }],
  ])("is not asked for, or shown, while %s", async (stage, patch) => {
    inSettings({ stage, member: null, ...patch });
    // The section has loaded once its own box is there.
    await waitFor(() =>
      expect(screen.getByRole("main").querySelector(".community")).not.toBeNull(),
    );
    expect(screen.queryByRole("heading", { level: 3, name: "Blocked" })).toBeNull();
    expect(api.communityBlocked).not.toHaveBeenCalled();
  });

  it("passes the accessibility smoke check, empty and filled", async () => {
    api.communityBlocked.mockResolvedValue([PAT_BLOCKED, KIM_BLOCKED]);
    const { container } = inSettings();
    await screen.findByRole("list", { name: "People you blocked" });
    expect(a11yProblems(container)).toEqual([]);
  });
});

describe("Settings → Community → Delete my Community data from this PC", () => {
  const BUTTON = "Delete my Community data from this PC";
  const ASK =
    "Delete every Community conversation and message on this PC? 8 West and the other people keep theirs. This can't be undone.";

  /** Everything else a screen here could call: none of it is for this. */
  const others = () => [
    api.blockInCommunity,
    api.unblockInCommunity,
    api.reportInCommunity,
    api.leaveCommunityConversation,
    api.setCommunitySwitch,
    api.signOutOfCommunity,
  ];

  it("asks first, in plain words, and calls nothing until you say yes", async () => {
    const { user } = inSettings();
    await user.click(await screen.findByRole("button", { name: BUTTON }));
    const ask = await dialog(`${BUTTON}?`);
    expect(within(ask).getByText(ASK)).toBeInTheDocument();
    expect(api.deleteMyCommunityData).not.toHaveBeenCalled();
    // No: nothing is deleted, and nothing is said about it.
    await user.click(within(ask).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.deleteMyCommunityData).not.toHaveBeenCalled();
    expect(screen.queryByText("Deleted.")).toBeNull();
  });

  it("deletes after you say yes, says Deleted., and calls nothing else", async () => {
    api.deleteMyCommunityData.mockResolvedValue(undefined);
    const { user } = inSettings();
    await user.click(await screen.findByRole("button", { name: BUTTON }));
    const ask = await dialog(`${BUTTON}?`);
    await user.click(within(ask).getByRole("button", { name: "Delete my Community data" }));
    expect(api.deleteMyCommunityData).toHaveBeenCalledExactlyOnceWith();
    expect(await screen.findByText("Deleted.")).toBeVisible();
    expect(screen.queryByRole("dialog")).toBeNull();
    for (const command of others()) expect(command).not.toHaveBeenCalled();
    // You are still signed in: the section is as it was.
    expect(screen.getByRole("button", { name: "Leave Community" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sign out of your account" })).toBeEnabled();
  });

  it("is a danger button", async () => {
    inSettings();
    expect(await screen.findByRole("button", { name: BUTTON })).toHaveClass("ui-button--danger");
  });

  it("says Deleted. no more when it is asked about again", async () => {
    api.deleteMyCommunityData.mockResolvedValue(undefined);
    const { user } = inSettings();
    await user.click(await screen.findByRole("button", { name: BUTTON }));
    await user.click(
      within(await dialog(`${BUTTON}?`)).getByRole("button", { name: "Delete my Community data" }),
    );
    await screen.findByText("Deleted.");
    await user.click(screen.getByRole("button", { name: BUTTON }));
    expect(screen.queryByText("Deleted.")).toBeNull();
  });

  it("says why when it can't, and stays asking", async () => {
    api.deleteMyCommunityData.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "Plenipo is still starting."),
    );
    const { user } = inSettings();
    await user.click(await screen.findByRole("button", { name: BUTTON }));
    const ask = await dialog(`${BUTTON}?`);
    await user.click(within(ask).getByRole("button", { name: "Delete my Community data" }));
    expect(await within(ask).findByRole("alert")).toHaveTextContent("Plenipo is still starting.");
    expect(screen.queryByText("Deleted.")).toBeNull();
  });

  it("says this Mac, on a Mac", async () => {
    setSystemWords(SYSTEM_WORDS.mac);
    const { user } = inSettings();
    await user.click(
      await screen.findByRole("button", { name: "Delete my Community data from this Mac" }),
    );
    expect(await dialog("Delete my Community data from this Mac?")).toHaveTextContent(
      "Delete every Community conversation and message on this Mac? 8 West and the other people keep theirs. This can't be undone.",
    );
  });

  it.each<[Stage, Partial<CommunityView>, boolean]>([
    ["off", {}, true],
    ["comingSoon", { comingSoon: true }, true],
    ["updateNeeded", {}, true],
    ["unreachable", { problem: "8 West is busy. Nothing was changed." }, true],
    ["signingIn", { code: "4KQ-7TD" }, false],
    ["joining", { terms: "2026-10-01" }, false],
    ["signedIn", {}, true],
    ["signedOut", {}, true],
    ["closed", {}, true],
  ])("in the %s stage, it is there: %s", async (stage, patch, there) => {
    inSettings({ stage, ...(stage === "signedIn" ? {} : { member: null }), ...patch });
    // The section has loaded once its own box is there.
    await waitFor(() =>
      expect(screen.getByRole("main").querySelector(".community")).not.toBeNull(),
    );
    expect(screen.queryByRole("button", { name: BUTTON }) !== null).toBe(there);
  });

  it("passes the accessibility smoke check, and asking", async () => {
    const { user, container } = inSettings();
    await user.click(await screen.findByRole("button", { name: BUTTON }));
    await dialog(`${BUTTON}?`);
    expect(a11yProblems(container)).toEqual([]);
  });
});
