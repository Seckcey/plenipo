import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { LedgerEvent, OwnerProfile } from "@plenipo/types";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import * as events from "../api/events";
import { a11yProblems } from "../test/a11y";
import { OwnerButton } from "./OwnerButton";
import { OwnerFace, OwnerStatusLine } from "./OwnerFace";
import { OwnerProvider } from "./OwnerProvider";
import * as picture from "./picture";
import { describeOwner, oneLineMessage } from "./words";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return { ...actual, getOwnerProfile: vi.fn(), setOwnerProfile: vi.fn() };
});
vi.mock("../api/events", () => ({ subscribeLedgerEvents: vi.fn(), subscribeShared: vi.fn() }));
vi.mock("./picture", async (importOriginal) => {
  const actual = await importOriginal<typeof picture>();
  return { ...actual, shrinkToPng: vi.fn() };
});

const api = vi.mocked(commands);
const shrink = vi.mocked(picture.shrinkToPng);
let emit: (e: LedgerEvent) => void = () => undefined;
let shared: (what: "tile" | "workforce") => void = () => undefined;

const PNG =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGBgAAAABQABpfZFQAAAAABJRU5ErkJggg==";

/** A promise the test answers when it wants to (a slow read or save). */
function later<T>() {
  let resolve: (value: T) => void = () => undefined;
  const promise = new Promise<T>((yes) => {
    resolve = yes;
  });
  return {
    promise,
    /** Answer it, and let the page take the answer in. */
    answer: (value: T) =>
      act(async () => {
        resolve(value);
        await promise;
      }),
  };
}

function profile(patch: Partial<OwnerProfile> = {}): OwnerProfile {
  return { status: "available", mood: null, message: "", picture: null, ...patch };
}

function ledgerEvent(eventType: string, seq = 1): LedgerEvent {
  return {
    seq,
    id: `e${seq}`,
    taskId: null,
    executionId: null,
    source: "owner",
    destination: null,
    eventType,
    payload: {},
    createdAt: 0,
  };
}

/** The button in a page (its heading and the top bar), with or without the provider. */
function renderButton(withProvider = true) {
  const button = withProvider ? (
    <OwnerProvider>
      <OwnerButton />
    </OwnerProvider>
  ) : (
    <OwnerButton />
  );
  return render(
    <main>
      <h1>Page</h1>
      <header>{button}</header>
      <p>Somewhere else</p>
    </main>,
  );
}

async function openPanel(name: RegExp | string = /^You: Available — change your picture/) {
  const user = userEvent.setup();
  await user.click(await screen.findByRole("button", { name }));
  const panel = screen.getByRole("dialog", { name: "You" });
  return { user, panel };
}

beforeEach(() => {
  api.getOwnerProfile.mockResolvedValue(profile());
  api.setOwnerProfile.mockImplementation((input) =>
    Promise.resolve(
      profile({
        status: input.status,
        mood: input.mood,
        message: input.message,
        picture: input.picture.kind === "set" ? input.picture.png : null,
      }),
    ),
  );
  vi.mocked(events.subscribeLedgerEvents).mockImplementation((handler) => {
    emit = handler;
    return Promise.resolve(() => undefined);
  });
  vi.mocked(events.subscribeShared).mockImplementation((handler) => {
    shared = handler;
    return Promise.resolve(() => undefined);
  });
});

describe("words", () => {
  it("keeps the message to one line of at most 80", () => {
    expect(oneLineMessage("Feeling\ngreat!\t")).toBe("Feeling great! ");
    expect(oneLineMessage("x".repeat(100))).toHaveLength(80);
    // A face at the end is never cut in two.
    expect(oneLineMessage(`${"x".repeat(79)}😄`)).toBe("x".repeat(79));
  });

  it("says your tile in words", () => {
    expect(describeOwner(null)).toBe("");
    expect(describeOwner(profile())).toBe("Available");
    expect(
      describeOwner(profile({ status: "doNotDisturb", mood: "tired", message: "Back at 3" })),
    ).toBe("Do not disturb, feeling Tired, “Back at 3”");
  });
});

describe("OwnerFace and OwnerStatusLine", () => {
  it("shows your picture, or the owner glyph, with the status light named", () => {
    const { container, rerender } = render(
      <OwnerFace profile={profile({ picture: PNG, status: "busy" })} size={40} />,
    );
    const img = screen.getByRole("img", { name: "Your picture" });
    expect(img).toHaveAttribute("src", `data:image/png;base64,${PNG}`);
    expect(screen.getByRole("img", { name: "Busy" })).toHaveAttribute("data-status", "busy");

    rerender(<OwnerFace profile={profile({ status: "away" })} />);
    expect(screen.queryByRole("img", { name: "Your picture" })).not.toBeInTheDocument();
    expect(container.querySelector(".owner-face__glyph svg")).not.toBeNull();
    expect(screen.getByRole("img", { name: "Away" })).toBeInTheDocument();

    // Before your tile is loaded: the glyph, and no light.
    rerender(<OwnerFace profile={null} />);
    expect(container.querySelector(".owner-light")).toBeNull();
  });

  it("gives every status and mood its word, never color or a face alone", () => {
    const { container, rerender } = render(
      <OwnerStatusLine
        profile={profile({ status: "doNotDisturb", mood: "tired", message: "Back at 3" })}
      />,
    );
    const line = container.querySelector(".owner-status") as HTMLElement;
    expect(line).toHaveTextContent("Do not disturb");
    expect(line).toHaveTextContent("Tired");
    expect(line).toHaveTextContent("Back at 3");
    expect(container.querySelector(".owner-status__face")).toHaveAttribute("aria-hidden", "true");
    expect(container.querySelector(".owner-light")).toHaveAttribute("aria-hidden", "true");
    // Only spans: it fits inside the canvas's tile, which is a button.
    expect(line.querySelectorAll(":not(span, svg, path)")).toHaveLength(0);

    rerender(<OwnerStatusLine profile={profile()} />);
    expect(container.querySelector(".owner-status")).toHaveTextContent(/^Available$/);
    expect(container.querySelector(".owner-status__mood")).toBeNull();
    expect(container.querySelector(".owner-status__message")).toBeNull();

    rerender(<OwnerStatusLine profile={null} />);
    expect(container.querySelector(".owner-status")).toBeNull();
  });
});

describe("OwnerButton", () => {
  it("opens the panel, and Escape closes it with the focus back on the button", async () => {
    renderButton();
    const { user, panel } = await openPanel();
    const button = screen.getByRole("button", { name: /^You: Available/ });
    expect(button).toHaveAttribute("aria-expanded", "true");
    expect(button).toHaveAttribute("aria-haspopup", "dialog");
    expect(within(panel).getByRole("button", { name: "Choose a picture…" })).toHaveFocus();
    expect(
      within(panel).getByText("Kept on this PC only. Plenipo never sends it anywhere."),
    ).toBeInTheDocument();
    for (const group of ["Your picture", "Status", "Mood"]) {
      expect(within(panel).getByRole("group", { name: group })).toBeInTheDocument();
    }
    expect(within(panel).getByRole("radio", { name: "Available" })).toBeChecked();
    expect(within(panel).getByRole("radio", { name: "None" })).toBeChecked();
    expect(a11yProblems(document.body)).toEqual([]);

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(button).toHaveFocus();
    expect(button).toHaveAttribute("aria-expanded", "false");
  });

  it("keeps Tab inside the panel, and a click outside closes it", async () => {
    renderButton();
    const { user, panel } = await openPanel();
    const choose = within(panel).getByRole("button", { name: "Choose a picture…" });
    await user.tab({ shift: true });
    expect(within(panel).getByRole("button", { name: "Save" })).toHaveFocus();
    await user.tab();
    expect(choose).toHaveFocus();
    await user.click(screen.getByText("Somewhere else"));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("saves your status, mood, and message, keeping the picture", async () => {
    renderButton();
    const { user, panel } = await openPanel();
    await user.click(within(panel).getByRole("radio", { name: "Busy" }));
    await user.click(within(panel).getByRole("radio", { name: "Great" }));
    await user.type(within(panel).getByRole("textbox", { name: "Message" }), "  Feeling great!  ");
    expect(within(panel).getByText("18 of 80")).toBeInTheDocument();
    await user.click(within(panel).getByRole("button", { name: "Save" }));

    expect(api.setOwnerProfile).toHaveBeenCalledWith({
      status: "busy",
      mood: "great",
      message: "Feeling great!",
      picture: { kind: "keep" },
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    const button = screen.getByRole("button", {
      name: "You: Busy, feeling Great — change your picture, status, mood, and message",
    });
    expect(button).toHaveFocus();
  });

  it("starts from your tile as kept, and Cancel keeps nothing", async () => {
    api.getOwnerProfile.mockResolvedValue(
      profile({ status: "away", mood: "focused", message: "Heads down", picture: PNG }),
    );
    renderButton();
    const { user, panel } = await openPanel(/^You: Away, feeling Focused/);
    expect(within(panel).getByRole("radio", { name: "Away" })).toBeChecked();
    expect(within(panel).getByRole("radio", { name: "Focused" })).toBeChecked();
    expect(within(panel).getByRole("textbox", { name: "Message" })).toHaveValue("Heads down");
    expect(within(panel).getByRole("img", { name: "Your picture" })).toHaveAttribute(
      "src",
      `data:image/png;base64,${PNG}`,
    );
    await user.click(within(panel).getByRole("radio", { name: "Busy" }));
    await user.click(within(panel).getByRole("button", { name: "Cancel" }));
    expect(api.setOwnerProfile).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: /^You: Away/ })).toHaveFocus();
  });

  it("Remove picture sends a removal", async () => {
    api.getOwnerProfile.mockResolvedValue(profile({ picture: PNG }));
    renderButton();
    const { user, panel } = await openPanel();
    await user.click(within(panel).getByRole("button", { name: "Remove picture" }));
    expect(within(panel).queryByRole("img", { name: "Your picture" })).not.toBeInTheDocument();
    expect(within(panel).queryByRole("button", { name: "Remove picture" })).not.toBeInTheDocument();
    expect(within(panel).getByRole("button", { name: "Choose a picture…" })).toHaveFocus();
    await user.click(within(panel).getByRole("button", { name: "Save" }));
    expect(api.setOwnerProfile).toHaveBeenCalledWith(
      expect.objectContaining({ picture: { kind: "remove" } }),
    );
  });

  it("a chosen picture is shrunk in the window and sent as a new PNG", async () => {
    shrink.mockResolvedValue(PNG);
    renderButton();
    const { user, panel } = await openPanel();
    const input = panel.querySelector<HTMLInputElement>('input[type="file"]');
    expect(input).not.toBeNull();
    expect(input).toHaveAttribute("accept", "image/png,image/jpeg,image/gif,image/webp,image/bmp");
    const click = vi.spyOn(input!, "click");
    await user.click(within(panel).getByRole("button", { name: "Choose a picture…" }));
    expect(click).toHaveBeenCalled();

    const file = new File(["picture"], "me.jpg", { type: "image/jpeg" });
    fireEvent.change(input!, { target: { files: [file] } });
    expect(await within(panel).findByRole("img", { name: "Your picture" })).toHaveAttribute(
      "src",
      `data:image/png;base64,${PNG}`,
    );
    expect(shrink).toHaveBeenCalledWith(file);
    await user.click(within(panel).getByRole("button", { name: "Save" }));
    expect(api.setOwnerProfile).toHaveBeenCalledWith(
      expect.objectContaining({ picture: { kind: "set", png: PNG } }),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    // The button shows the new picture.
    expect(
      within(screen.getByRole("button", { name: /^You: Available/ })).getByRole("img", {
        name: "Your picture",
      }),
    ).toBeInTheDocument();
  });

  it("says in plain words why a chosen file can't be used", async () => {
    shrink.mockRejectedValue(new picture.PictureError(picture.NOT_A_PICTURE));
    renderButton();
    const { panel } = await openPanel();
    const input = panel.querySelector<HTMLInputElement>('input[type="file"]')!;
    fireEvent.change(input, { target: { files: [new File(["x"], "notes.txt")] } });
    expect(await within(panel).findByRole("alert")).toHaveTextContent(picture.NOT_A_PICTURE);
    expect(within(panel).queryByRole("img", { name: "Your picture" })).not.toBeInTheDocument();
  });

  it("caps the message at 80 characters, on one line", async () => {
    renderButton();
    const { user, panel } = await openPanel();
    const box = within(panel).getByRole("textbox", { name: "Message" });
    expect(box).toHaveAttribute("maxLength", "80");
    await user.type(box, "y".repeat(100));
    expect(box).toHaveValue("y".repeat(80));
    expect(within(panel).getByText("80 of 80")).toBeInTheDocument();
    // A pasted tab becomes a space (the text box itself drops line breaks).
    fireEvent.change(box, { target: { value: `two\tlines${"z".repeat(90)}` } });
    expect((box as HTMLInputElement).value).toHaveLength(80);
    expect((box as HTMLInputElement).value.startsWith("two lines")).toBe(true);
  });

  it("explains Do not disturb when you choose it", async () => {
    renderButton();
    const { user, panel } = await openPanel();
    const hint =
      "Windows pop-up notices wait while this is on and come as one when you turn it off; the bell still counts them.";
    expect(within(panel).queryByText(hint)).not.toBeInTheDocument();
    const dnd = within(panel).getByRole("radio", { name: "Do not disturb" });
    await user.click(dnd);
    expect(within(panel).getByText(hint)).toBeInTheDocument();
    expect(dnd).toHaveAccessibleDescription(hint);
    expect(a11yProblems(document.body)).toEqual([]);
  });

  it("shows why Plenipo refused a change, and keeps the panel open", async () => {
    api.setOwnerProfile.mockRejectedValue(
      new commands.PlenipoCommandError(
        "invalidInput",
        "your message must be one line of plain text",
      ),
    );
    renderButton();
    const { user, panel } = await openPanel();
    await user.click(within(panel).getByRole("button", { name: "Save" }));
    expect(await within(panel).findByRole("alert")).toHaveTextContent(
      "Couldn't save your changes: your message must be one line of plain text",
    );
    expect(screen.getByRole("dialog", { name: "You" })).toBeInTheDocument();
    expect(within(panel).getByRole("button", { name: "Save" })).toBeEnabled();
  });

  it("works without a provider: nothing loaded, and Save stays off", async () => {
    renderButton(false);
    const { panel } = await openPanel("You — change your picture, status, mood, and message");
    expect(within(panel).getByRole("alert")).toHaveTextContent(
      "Your picture, status, mood, and message can't change here.",
    );
    expect(within(panel).getByRole("button", { name: "Save" })).toBeDisabled();
    expect(within(panel).queryByRole("button", { name: "Try again" })).not.toBeInTheDocument();
    expect(api.getOwnerProfile).not.toHaveBeenCalled();
    expect(api.setOwnerProfile).not.toHaveBeenCalled();
  });

  it("keeps Save off until your details are read, reading them again when the panel opens", async () => {
    // The first read, when Plenipo starts, fails.
    api.getOwnerProfile.mockRejectedValueOnce(new commands.PlenipoCommandError("internal", "down"));
    const second = later<OwnerProfile>();
    api.getOwnerProfile.mockReturnValueOnce(second.promise);
    renderButton();
    await waitFor(() => expect(api.getOwnerProfile).toHaveBeenCalledTimes(1));
    const { panel } = await openPanel("You — change your picture, status, mood, and message");
    // Opening the panel reads them again; meanwhile Save would wipe what is kept, so it is off.
    expect(api.getOwnerProfile).toHaveBeenCalledTimes(2);
    expect(within(panel).getByRole("status")).toHaveTextContent("Reading your details…");
    expect(within(panel).getByRole("button", { name: "Save" })).toBeDisabled();
    expect(a11yProblems(document.body)).toEqual([]);

    await second.answer(profile({ status: "away", mood: "focused", message: "Heads down" }));
    // They fill the panel as they arrive.
    expect(within(panel).queryByText("Reading your details…")).not.toBeInTheDocument();
    expect(within(panel).getByRole("radio", { name: "Away" })).toBeChecked();
    expect(within(panel).getByRole("radio", { name: "Focused" })).toBeChecked();
    expect(within(panel).getByRole("textbox", { name: "Message" })).toHaveValue("Heads down");
    expect(within(panel).getByRole("button", { name: "Save" })).toBeEnabled();
  });

  it("fills in only what you have not changed when your details arrive", async () => {
    const first = later<OwnerProfile>();
    api.getOwnerProfile.mockReturnValue(first.promise);
    renderButton();
    await waitFor(() => expect(api.getOwnerProfile).toHaveBeenCalled());
    const { user, panel } = await openPanel("You — change your picture, status, mood, and message");
    await user.click(within(panel).getByRole("radio", { name: "Busy" }));
    await user.type(within(panel).getByRole("textbox", { name: "Message" }), "Back at 3");
    // Every read answers with the same details (opening the panel read them again).
    await first.answer(profile({ status: "away", mood: "focused", message: "Heads down" }));
    expect(within(panel).getByRole("radio", { name: "Busy" })).toBeChecked();
    expect(within(panel).getByRole("radio", { name: "Focused" })).toBeChecked();
    expect(within(panel).getByRole("textbox", { name: "Message" })).toHaveValue("Back at 3");
    await user.click(within(panel).getByRole("button", { name: "Save" }));
    expect(api.setOwnerProfile).toHaveBeenCalledWith({
      status: "busy",
      mood: "focused",
      message: "Back at 3",
      picture: { kind: "keep" },
    });
  });

  it("says when your details could not be read, and Try again reads them again", async () => {
    api.getOwnerProfile.mockRejectedValue(new commands.PlenipoCommandError("internal", "down"));
    renderButton();
    await waitFor(() => expect(api.getOwnerProfile).toHaveBeenCalledTimes(1));
    const { user, panel } = await openPanel("You — change your picture, status, mood, and message");
    expect(await within(panel).findByRole("alert")).toHaveTextContent(
      "Plenipo couldn't read your details, so Save is off for now: down",
    );
    expect(within(panel).getByRole("button", { name: "Save" })).toBeDisabled();
    api.getOwnerProfile.mockResolvedValue(profile({ status: "busy" }));
    await user.click(within(panel).getByRole("button", { name: "Try again" }));
    expect(within(panel).getByRole("button", { name: "Choose a picture…" })).toHaveFocus();
    await waitFor(() => expect(within(panel).getByRole("radio", { name: "Busy" })).toBeChecked());
    expect(within(panel).queryByRole("alert")).not.toBeInTheDocument();
    expect(within(panel).getByRole("button", { name: "Save" })).toBeEnabled();
  });

  it("waits for a slow save: Cancel, Escape, and clicks outside do not close the panel", async () => {
    const slow = later<OwnerProfile>();
    api.setOwnerProfile.mockReturnValue(slow.promise);
    renderButton();
    const { user, panel } = await openPanel();
    await user.click(within(panel).getByRole("radio", { name: "Busy" }));
    await user.click(within(panel).getByRole("button", { name: "Save" }));
    expect(within(panel).getByRole("button", { name: "Saving…" })).toBeDisabled();
    expect(within(panel).getByRole("button", { name: "Cancel" })).toBeDisabled();
    fireEvent.keyDown(within(panel).getByRole("textbox", { name: "Message" }), { key: "Escape" });
    await user.click(screen.getByText("Somewhere else"));
    await user.click(screen.getByRole("button", { name: /^You: Available/ }));
    expect(screen.getByRole("dialog", { name: "You" })).toBe(panel);

    await slow.answer(profile({ status: "busy" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: /^You: Busy/ })).toHaveFocus();
  });

  it("keeps Shift+Tab inside the panel while the picture is being read", async () => {
    const reading = later<string>();
    shrink.mockReturnValue(reading.promise);
    renderButton();
    const { user, panel } = await openPanel();
    const busy = within(panel).getByRole("radio", { name: "Busy" });
    await user.click(busy);
    const input = panel.querySelector<HTMLInputElement>('input[type="file"]')!;
    fireEvent.change(input, {
      target: { files: [new File(["x"], "me.png", { type: "image/png" })] },
    });
    expect(
      await within(panel).findByRole("button", { name: "Reading the picture…" }),
    ).toBeDisabled();
    // The chosen status is the first stop Tab can reach: Shift+Tab goes round to the last.
    busy.focus();
    await user.tab({ shift: true });
    const cancel = within(panel).getByRole("button", { name: "Cancel" });
    expect(cancel).toHaveFocus();
    await user.tab();
    expect(busy).toHaveFocus();
    await reading.answer(PNG);
  });
});

describe("OwnerProvider", () => {
  it("reads your tile again when the Ledger records a change", async () => {
    renderButton();
    await screen.findByRole("button", { name: /^You: Available — / });
    expect(api.getOwnerProfile).toHaveBeenCalledTimes(1);

    act(() => emit(ledgerEvent("task.state_changed", 1)));
    expect(api.getOwnerProfile).toHaveBeenCalledTimes(1);

    api.getOwnerProfile.mockResolvedValue(profile({ status: "doNotDisturb", mood: "celebrating" }));
    act(() => emit(ledgerEvent("owner.profile_changed", 2)));
    expect(
      await screen.findByRole("button", {
        name: "You: Do not disturb, feeling Celebrating — change your picture, status, mood, and message",
      }),
    ).toBeInTheDocument();
    expect(api.getOwnerProfile).toHaveBeenCalledTimes(2);
  });

  it("reads your tile again when another organization's window changes it", async () => {
    renderButton();
    await screen.findByRole("button", { name: /^You: Available — / });
    act(() => shared("workforce"));
    expect(api.getOwnerProfile).toHaveBeenCalledTimes(1);
    api.getOwnerProfile.mockResolvedValue(profile({ status: "busy" }));
    act(() => shared("tile"));
    expect(await screen.findByRole("button", { name: /^You: Busy — / })).toBeInTheDocument();
  });

  it("stays quiet when your tile can't be read", async () => {
    api.getOwnerProfile.mockRejectedValue(new commands.PlenipoCommandError("internal", "down"));
    vi.mocked(events.subscribeLedgerEvents).mockRejectedValue(new Error("no events"));
    renderButton();
    await waitFor(() => expect(api.getOwnerProfile).toHaveBeenCalled());
    expect(
      screen.getByRole("button", { name: "You — change your picture, status, mood, and message" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
