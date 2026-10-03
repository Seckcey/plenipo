import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { CommunityView, OwnerProfile, ProfileDraft } from "@plenipo/types";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { OwnerContext } from "../owner/context";
import { a11yProblems } from "../test/a11y";
import { communityView, profileDraft } from "../test/communityFixtures";
import { CommunitySettings } from "./CommunitySettings";
import { BUSINESS_KINDS, PARTS, regionLabel, regionOptions } from "./profileWords";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getCommunity: vi.fn(),
    joinCommunity: vi.fn(),
    saveCommunityProfile: vi.fn(),
    setCommunityAppearOffline: vi.fn(),
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

const PNG =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGBgAAAABQABpfZFQAAAAABJRU5ErkJggg==";

/** Your tile: Do not disturb, focused, a message, and a picture. */
function tile(patch: Partial<OwnerProfile> = {}): OwnerProfile {
  return {
    status: "doNotDisturb",
    mood: "focused",
    message: "Back at 3",
    picture: PNG,
    ...patch,
  };
}

const ALL_PARTS = PARTS.map((p) => p.label);
const joining = communityView({
  stage: "joining",
  accountName: "Frank Gonzalez",
  terms: "2026-10-01",
  profile: profileDraft({ displayName: "Frank Gonzalez" }),
});
const member = {
  name: "pat-lee",
  adult: true,
  canStart: false,
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
  profile: profileDraft({ displayName: "Pat Lee", company: "Lee Builders" }),
});

/** The Community page, with your tile as the top bar keeps it. */
function inPage(view: CommunityView, owner: OwnerProfile | null = tile()) {
  api.getCommunity.mockResolvedValue(view);
  return render(
    <OwnerContext.Provider
      value={{ profile: owner, save: () => Promise.reject(new Error("not here")) }}
    >
      <main>
        <h1>Settings</h1>
        <h2>Community</h2>
        <CommunitySettings go={go} />
      </main>
    </OwnerContext.Provider>,
  );
}

const preview = () => screen.findByRole("region", { name: "What people see" });
const box = (name: string) => screen.getByRole("checkbox", { name });
const kinds = () => within(screen.getByRole("group", { name: "What your business does" }));

beforeEach(() => {
  vi.clearAllMocks();
  changed.current = () => undefined;
});

describe("the words for your profile", () => {
  it("uses the contract's kinds of business, with a plain word for each", () => {
    // The wire values are the contract's own (`BusinessKind`): a drift is a refused profile.
    const schema = JSON.parse(
      readFileSync(
        resolve(process.cwd(), "../../contracts/community/v1/schema/community.schema.json"),
        "utf8",
      ),
    ) as { $defs: { BusinessKind: { enum: string[] } } };
    expect(BUSINESS_KINDS.map((k) => k.value)).toEqual(schema.$defs.BusinessKind.enum);
    expect(BUSINESS_KINDS).toHaveLength(30);
    expect(new Set(BUSINESS_KINDS.map((k) => k.label)).size).toBe(30);
  });

  it("lists Not shown, the 50 states and DC, and then the countries", () => {
    const all = regionOptions("");
    expect(all[0]).toEqual({ value: "", label: "Not shown" });
    const states = all.filter((o) => o.group === "States");
    expect(states).toHaveLength(51);
    expect(states.find((o) => o.value === "US-CA")?.label).toBe("California (United States)");
    expect(states.find((o) => o.value === "US-DC")?.label).toBe(
      "District of Columbia (United States)",
    );
    const countries = all.filter((o) => o.group === "Countries");
    expect(countries.find((o) => o.value === "CA")?.label).toBe("Canada");
    expect(countries.find((o) => o.value === "US")?.label).toBe("United States");
    // Every place is one the contract takes, and every country has a name, not just a code.
    for (const o of all.filter((o) => o.value !== "")) {
      expect(o.value).toMatch(/^[A-Z]{2}(-[A-Z0-9]{1,3})?$/);
    }
    for (const o of countries) expect(o.label).not.toBe(o.value);
    expect(new Set(all.map((o) => o.value)).size).toBe(all.length);
  });

  it("keeps a place that is kept but not in the list, so the box shows what is kept", () => {
    expect(regionOptions("US-PR").at(-1)).toEqual({
      value: "US-PR",
      label: "US-PR",
      group: "Other places",
    });
    expect(regionLabel("US-PR")).toBe("US-PR");
    expect(regionLabel("FR")).toBe("France");
  });
});

describe("What people see, when you join", () => {
  it("shows the boxes, all ticked, and the card of what people see", async () => {
    const { container } = inPage(joining);
    const card = await preview();
    for (const label of ALL_PARTS) expect(box(label)).toBeChecked();
    expect(ALL_PARTS).toEqual([
      "Show my picture",
      "Show my name",
      "Show my status",
      "Show my mood",
      "Show my message",
      "Show my company",
      "Show what my business does",
      "Show where I am",
    ]);
    const name = screen.getByRole("textbox", { name: "Your name" });
    expect(name).toHaveValue("Frank Gonzalez");
    expect(name).toHaveAttribute("maxLength", "60");
    expect(screen.getByRole("textbox", { name: "Company" })).toHaveAttribute("maxLength", "80");
    expect(screen.getByRole("textbox", { name: "In your own words" })).toHaveAttribute(
      "maxLength",
      "80",
    );
    const where = screen.getByRole("combobox", { name: "Where" });
    expect(where).toHaveValue("");
    expect(within(where).getAllByRole("option")[0]).toHaveTextContent("Not shown");
    // The card holds your tile: picture, status, mood, message, and your name.
    expect(within(card).getByRole("heading", { name: "What people see" })).toBeVisible();
    expect(within(card).getByRole("img", { name: "Your picture" })).toHaveAttribute(
      "src",
      `data:image/png;base64,${PNG}`,
    );
    expect(within(card).getByText("Frank Gonzalez")).toBeVisible();
    expect(within(card).getByText("Focused")).toBeVisible();
    expect(within(card).getByText("Back at 3")).toBeVisible();
    // The notice comes first, then the card.
    expect(screen.getByRole("note")).toHaveTextContent(
      "You'll be listed in the Community directory",
    );
    expect(a11yProblems(container)).toEqual([]);
  });

  it("sends the profile you made when you join", async () => {
    const user = userEvent.setup();
    inPage(joining);
    api.joinCommunity.mockResolvedValue(signedIn);
    await screen.findByRole("button", { name: "Join Community" });
    await user.type(screen.getByRole("textbox", { name: "Your name in Community" }), "pat-lee");
    await user.selectOptions(screen.getByRole("combobox", { name: "Birth month" }), "April");
    await user.selectOptions(screen.getByRole("combobox", { name: "Birth year" }), "1990");
    await user.click(screen.getByRole("checkbox", { name: "I agree to the Community terms" }));

    const company = screen.getByRole("textbox", { name: "Company" });
    await user.type(company, "Lee Builders");
    await user.click(kinds().getByRole("checkbox", { name: "Construction" }));
    await user.click(kinds().getByRole("checkbox", { name: "Trades" }));
    await user.type(
      screen.getByRole("textbox", { name: "In your own words" }),
      "Homes and repairs",
    );
    await user.selectOptions(screen.getByRole("combobox", { name: "Where" }), "US-CA");
    await user.click(box("Show my mood"));
    await user.click(screen.getByRole("button", { name: "Join Community" }));

    const expected: ProfileDraft = profileDraft({
      displayName: "Frank Gonzalez",
      company: "Lee Builders",
      businessKinds: ["construction", "trades"],
      businessLine: "Homes and repairs",
      region: "US-CA",
    });
    expected.shown.mood = false;
    expect(api.joinCommunity).toHaveBeenCalledExactlyOnceWith(
      "pat-lee",
      4,
      1990,
      "2026-10-01",
      expected,
    );
    expect(await screen.findByText("@pat-lee")).toBeInTheDocument();
  });

  it("starts from the profile Community already keeps", async () => {
    inPage({
      ...joining,
      profile: profileDraft({
        displayName: "Pat Lee",
        company: "Lee Builders",
        businessKinds: ["construction"],
        businessLine: "Homes and repairs",
        region: "US-CA",
        shown: { ...profileDraft().shown, company: false },
      }),
    });
    await preview();
    expect(screen.getByRole("textbox", { name: "Your name" })).toHaveValue("Pat Lee");
    expect(screen.getByRole("textbox", { name: "Company" })).toHaveValue("Lee Builders");
    expect(kinds().getByRole("checkbox", { name: "Construction" })).toBeChecked();
    expect(screen.getByRole("combobox", { name: "Where" })).toHaveValue("US-CA");
    expect(box("Show my company")).not.toBeChecked();
    expect(box("Show my name")).toBeChecked();
  });
});

describe("the card of what people see", () => {
  it("shows only the parts that are ticked, and nothing that is empty", async () => {
    const user = userEvent.setup();
    inPage(joining);
    const card = await preview();
    // Nothing typed for the company, the kinds, the line, or where: not on the card.
    expect(card).not.toHaveTextContent("Not shown");
    expect(card.querySelector(".what-people-see__company")).toBeNull();
    expect(card.querySelector(".what-people-see__business")).toBeNull();
    expect(card.querySelector(".what-people-see__line")).toBeNull();
    expect(card.querySelector(".what-people-see__place")).toBeNull();

    await user.type(screen.getByRole("textbox", { name: "Company" }), "Lee Builders");
    await user.click(kinds().getByRole("checkbox", { name: "Construction" }));
    await user.click(kinds().getByRole("checkbox", { name: "Shops" }));
    await user.type(screen.getByRole("textbox", { name: "In your own words" }), "Homes and shops");
    await user.selectOptions(screen.getByRole("combobox", { name: "Where" }), "US-CA");
    expect(within(card).getByText("Lee Builders")).toBeVisible();
    expect(within(card).getByText("Construction, Shops")).toBeVisible();
    expect(within(card).getByText("Homes and shops")).toBeVisible();
    expect(within(card).getByText("California (United States)")).toBeVisible();

    // Unticked: gone from the card, though still typed.
    await user.click(box("Show my company"));
    expect(within(card).queryByText("Lee Builders")).toBeNull();
    expect(screen.getByRole("textbox", { name: "Company" })).toHaveValue("Lee Builders");
    await user.click(box("Show what my business does"));
    expect(within(card).queryByText("Construction, Shops")).toBeNull();
    expect(within(card).queryByText("Homes and shops")).toBeNull();
    await user.click(box("Show where I am"));
    expect(within(card).queryByText("California (United States)")).toBeNull();
    // Ticked again: back.
    await user.click(box("Show where I am"));
    expect(within(card).getByText("California (United States)")).toBeVisible();
  });

  it("hides your message when you untick it, and the rest stays", async () => {
    const user = userEvent.setup();
    inPage(joining);
    const card = await preview();
    expect(within(card).getByText("Back at 3")).toBeVisible();
    await user.click(box("Show my message"));
    expect(within(card).queryByText("Back at 3")).toBeNull();
    expect(within(card).getByText("Busy")).toBeVisible();
    expect(within(card).getByText("Focused")).toBeVisible();
    await user.click(box("Show my picture"));
    expect(within(card).queryByRole("img", { name: "Your picture" })).toBeNull();
    await user.click(box("Show my mood"));
    expect(within(card).queryByText("Focused")).toBeNull();
    await user.click(box("Show my status"));
    expect(within(card).queryByText("Busy")).toBeNull();
    await user.click(box("Show my name"));
    expect(within(card).queryByText("Frank Gonzalez")).toBeNull();
    // Everything is unticked: the card says so, and shows nothing.
    expect(card).toHaveTextContent("Nothing to show yet.");
    await user.click(box("Show my message"));
    expect(within(card).getByText("Back at 3")).toBeVisible();
    expect(card).not.toHaveTextContent("Nothing to show yet.");
  });

  it("shows Do not disturb as Busy (ADR-163 §8)", async () => {
    inPage(joining, tile({ status: "doNotDisturb" }));
    const card = await preview();
    expect(within(card).getByText("Busy")).toBeVisible();
    expect(card).not.toHaveTextContent("Do not disturb");
    expect(card.querySelector(".owner-light--busy")).not.toBeNull();
  });

  it.each([
    ["available", "Available"],
    ["busy", "Busy"],
    ["away", "Away"],
  ] as const)("shows %s as %s", async (status, word) => {
    inPage(joining, tile({ status, mood: null, message: "", picture: null }));
    const card = await preview();
    expect(within(card).getByText(word)).toBeVisible();
    // No mood, message, or picture on your tile: none on the card.
    expect(card).not.toHaveTextContent(/Great|Good|Okay|Tired|Stressed|Focused|Celebrating/);
    expect(card.querySelector(".what-people-see__message")).toBeNull();
    expect(card.querySelector("img")).toBeNull();
  });

  it("shows only your typed parts while your tile is not read yet", async () => {
    inPage(joining, null);
    const card = await preview();
    expect(within(card).getByText("Frank Gonzalez")).toBeVisible();
    expect(card.querySelector("img")).toBeNull();
    expect(card.querySelector(".what-people-see__tile")).toBeNull();
    expect(card.querySelector(".what-people-see__message")).toBeNull();
  });

  it("never shows a hidden character or a web page from your words, only plain text", async () => {
    const hidden = "\u202E";
    inPage(
      {
        ...joining,
        profile: profileDraft({
          displayName: `Pat${hidden}Lee <b>bold</b>`,
          company: `Lee${hidden}Builders\u200B`,
          businessLine: `Homes${hidden}and repairs`,
          region: "US-CA",
        }),
      },
      tile({ message: `Back${hidden}at 3\u200B\nnow`, mood: null, picture: null }),
    );
    const card = await preview();
    const text = card.textContent ?? "";
    for (const char of [hidden, "\u200B", "\n"]) expect(text).not.toContain(char);
    // Each one is a visible mark instead, so a person can see that something was there.
    expect(within(card).getByText("Pat\uFFFDLee <b>bold</b>")).toBeVisible();
    expect(within(card).getByText("Lee\uFFFDBuilders\uFFFD")).toBeVisible();
    expect(within(card).getByText("Homes\uFFFDand repairs")).toBeVisible();
    expect(within(card).getByText("Back\uFFFDat 3\uFFFD\uFFFDnow")).toBeVisible();
    // A web page typed in a text box stays text: no tag, no link.
    expect(card.querySelector("b, a")).toBeNull();
  });

  it("turns a tab you paste into a space", async () => {
    const user = userEvent.setup();
    inPage(joining);
    await preview();
    const company = screen.getByRole("textbox", { name: "Company" });
    await user.click(company);
    await user.paste("Lee\tBuilders");
    expect(company).toHaveValue("Lee Builders");
  });
});

describe("what your business does", () => {
  it("lists the kinds in plain words", async () => {
    inPage(joining);
    await preview();
    const labels = kinds()
      .getAllByRole("checkbox")
      .map((c) => c.closest("label")?.textContent);
    expect(labels).toEqual(BUSINESS_KINDS.map((k) => k.label));
    expect(labels).toContain("Cars and trucks");
    expect(labels).toContain("Hotels and travel stays");
    expect(labels).toContain("Phones and internet");
    expect(kinds().getByRole("checkbox", { name: "Farming" })).not.toBeChecked();
  });

  it("lets you choose at most 3 kinds", async () => {
    const user = userEvent.setup();
    inPage(joining);
    await preview();
    const picked = () =>
      kinds()
        .getAllByRole("checkbox")
        .filter((c) => (c as HTMLInputElement).checked);
    for (const name of ["Accounting", "Design", "Legal"]) {
      await user.click(kinds().getByRole("checkbox", { name }));
    }
    expect(picked()).toHaveLength(3);
    // Once 3 are chosen the rest are turned off, and the chosen ones can still be unticked.
    const others = kinds()
      .getAllByRole("checkbox")
      .filter((c) => !(c as HTMLInputElement).checked);
    expect(others).toHaveLength(27);
    for (const other of others) expect(other).toBeDisabled();
    for (const one of picked()) expect(one).toBeEnabled();
    await user.click(kinds().getByRole("checkbox", { name: "Shops" }));
    expect(kinds().getByRole("checkbox", { name: "Shops" })).not.toBeChecked();

    await user.click(kinds().getByRole("checkbox", { name: "Design" }));
    expect(picked()).toHaveLength(2);
    expect(kinds().getByRole("checkbox", { name: "Shops" })).toBeEnabled();
    await user.click(kinds().getByRole("checkbox", { name: "Shops" }));
    expect(picked()).toHaveLength(3);
  });

  it("starts with 3 kinds already chosen when 3 are kept", async () => {
    inPage({
      ...joining,
      profile: profileDraft({ businessKinds: ["legal", "design", "trades"] }),
    });
    await preview();
    expect(kinds().getByRole("checkbox", { name: "Legal" })).toBeEnabled();
    expect(kinds().getByRole("checkbox", { name: "Farming" })).toBeDisabled();
  });
});

describe("Your profile, once you are in", () => {
  it("shows what people see, with Save off until you change something", async () => {
    const { container } = inPage(signedIn);
    const card = await preview();
    expect(screen.getByRole("heading", { name: "Your profile" })).toBeVisible();
    expect(within(card).getByText("Pat Lee")).toBeVisible();
    expect(within(card).getByText("Lee Builders")).toBeVisible();
    for (const label of ALL_PARTS) expect(box(label)).toBeChecked();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    expect(a11yProblems(container)).toEqual([]);
  });

  it("saves the profile you changed, and then Save is off again", async () => {
    const user = userEvent.setup();
    inPage(signedIn);
    await preview();
    const save = screen.getByRole("button", { name: "Save" });
    const company = screen.getByRole("textbox", { name: "Company" });
    await user.clear(company);
    await user.type(company, "Lee and Sons");
    await user.click(kinds().getByRole("checkbox", { name: "Construction" }));
    await user.selectOptions(screen.getByRole("combobox", { name: "Where" }), "CA");
    await user.click(box("Show my message"));
    expect(save).toBeEnabled();
    expect(api.saveCommunityProfile).not.toHaveBeenCalled();

    const edited = profileDraft({
      displayName: "Pat Lee",
      company: "Lee and Sons",
      businessKinds: ["construction"],
      region: "CA",
    });
    edited.shown.message = false;
    api.saveCommunityProfile.mockResolvedValue({ ...signedIn, profile: edited });
    await user.click(save);
    expect(api.saveCommunityProfile).toHaveBeenCalledExactlyOnceWith(edited);
    await waitFor(() => expect(save).toBeDisabled());
    expect(screen.getByRole("textbox", { name: "Company" })).toHaveValue("Lee and Sons");
    expect(box("Show my message")).not.toBeChecked();
  });

  it("goes back to Save off when you change it back", async () => {
    const user = userEvent.setup();
    inPage(signedIn);
    await preview();
    await user.click(box("Show my mood"));
    expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
    await user.click(box("Show my mood"));
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  });

  it("says why saving did not work, and keeps what you typed", async () => {
    const user = userEvent.setup();
    inPage(signedIn);
    await preview();
    api.saveCommunityProfile.mockRejectedValue({
      kind: "invalidInput",
      message: "Your company can be at most 80 characters.",
    });
    await user.type(screen.getByRole("textbox", { name: "Company" }), " and Sons");
    await user.click(screen.getByRole("button", { name: "Save" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Your company can be at most 80 characters.");
    expect(alert).toHaveClass("form-error");
    expect(screen.getByRole("textbox", { name: "Company" })).toHaveValue("Lee Builders and Sons");
    expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
  });

  it("turns everything off while it saves", async () => {
    const user = userEvent.setup();
    inPage(signedIn);
    await preview();
    let done: (view: CommunityView) => void = () => undefined;
    api.saveCommunityProfile.mockReturnValue(
      new Promise<CommunityView>((resolve) => {
        done = resolve;
      }),
    );
    await user.click(box("Show my mood"));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    expect(screen.getByRole("textbox", { name: "Company" })).toBeDisabled();
    expect(screen.getByRole("combobox", { name: "Where" })).toBeDisabled();
    expect(box("Show my mood")).toBeDisabled();
    expect(kinds().getByRole("checkbox", { name: "Construction" })).toBeDisabled();
    done(signedIn);
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Company" })).toBeEnabled());
  });

  it("starts again from the kept profile when another window saved one", async () => {
    inPage(signedIn);
    await preview();
    expect(screen.getByRole("textbox", { name: "Company" })).toHaveValue("Lee Builders");
    // Another window saves a different company: Community changes, and this screen follows.
    api.getCommunity.mockResolvedValue({
      ...signedIn,
      profile: { ...signedIn.profile, company: "Lee and Sons" },
    });
    act(() => changed.current());
    await waitFor(() =>
      expect(screen.getByRole("textbox", { name: "Company" })).toHaveValue("Lee and Sons"),
    );
  });
});

describe("Appear offline, in Settings → Community", () => {
  const hint =
    "You leave the directory, New this week, and the leaderboard. People you talk with see you as Offline. Your messages keep working.";

  it("is off to begin with, with its hint and no line about it", async () => {
    inPage(signedIn);
    await preview();
    const offline = screen.getByRole("checkbox", { name: "Appear offline" });
    expect(offline).not.toBeChecked();
    expect(screen.getByText(hint)).toBeVisible();
    expect(screen.queryByText("You appear offline in Community.")).toBeNull();
  });

  it("turns on at once, and says that you appear offline", async () => {
    const user = userEvent.setup();
    inPage(signedIn);
    await preview();
    api.setCommunityAppearOffline.mockResolvedValue({
      ...signedIn,
      member: { ...member, appearOffline: true },
    });
    await user.click(screen.getByRole("checkbox", { name: "Appear offline" }));
    expect(api.setCommunityAppearOffline).toHaveBeenCalledExactlyOnceWith(true);
    expect(await screen.findByText("You appear offline in Community.")).toBeVisible();
    expect(screen.getByRole("checkbox", { name: "Appear offline" })).toBeChecked();
    // It is not part of Save: the profile was not sent.
    expect(api.saveCommunityProfile).not.toHaveBeenCalled();

    api.setCommunityAppearOffline.mockResolvedValue(signedIn);
    await user.click(screen.getByRole("checkbox", { name: "Appear offline" }));
    expect(api.setCommunityAppearOffline).toHaveBeenLastCalledWith(false);
    await waitFor(() => expect(screen.queryByText("You appear offline in Community.")).toBeNull());
    expect(screen.getByRole("checkbox", { name: "Appear offline" })).not.toBeChecked();
  });

  it("says when it could not change, and stays as it was", async () => {
    const user = userEvent.setup();
    inPage(signedIn);
    await preview();
    api.setCommunityAppearOffline.mockRejectedValue({
      kind: "internal",
      message: "Community can't be reached right now. Nothing was changed.",
    });
    await user.click(screen.getByRole("checkbox", { name: "Appear offline" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Community can't be reached right now. Nothing was changed.",
    );
    expect(screen.getByRole("checkbox", { name: "Appear offline" })).not.toBeChecked();
    expect(screen.queryByText("You appear offline in Community.")).toBeNull();
  });

  it("is not shown before you join", async () => {
    inPage(joining);
    await preview();
    expect(screen.queryByRole("checkbox", { name: "Appear offline" })).toBeNull();
  });
});

describe("a part 8 West hid", () => {
  it("says so in plain words, and keeps the part off the card", async () => {
    inPage(
      {
        ...signedIn,
        member: { ...member, hiddenParts: ["picture", "business_line"] },
        profile: profileDraft({
          displayName: "Pat Lee",
          businessKinds: ["construction"],
          businessLine: "Homes and repairs",
        }),
      },
      tile({ picture: PNG }),
    );
    const card = await preview();
    const notes = screen.getAllByRole("note").map((n) => n.textContent);
    expect(notes).toEqual([
      "8 West hid your picture because it broke the Community rules.",
      "8 West hid your line about your business because it broke the Community rules.",
    ]);
    // Not on the card, though ticked; what 8 West did not hide still is.
    expect(box("Show my picture")).toBeChecked();
    expect(within(card).queryByRole("img", { name: "Your picture" })).toBeNull();
    expect(within(card).queryByText("Homes and repairs")).toBeNull();
    expect(within(card).getByText("Construction")).toBeVisible();
    expect(within(card).getByText("Pat Lee")).toBeVisible();
    expect(within(card).getByText("Back at 3")).toBeVisible();
  });

  it.each([
    ["message", "message", "Back at 3"],
    ["display_name", "name", "Pat Lee"],
    ["company", "company", "Lee Builders"],
  ])("hides the %s from the card and names it %s", async (part, word, text) => {
    inPage({ ...signedIn, member: { ...member, hiddenParts: [part] } });
    const card = await preview();
    expect(screen.getByRole("note")).toHaveTextContent(
      `8 West hid your ${word} because it broke the Community rules.`,
    );
    expect(within(card).queryByText(text)).toBeNull();
  });

  it("says nothing when 8 West hid nothing, or a part this Plenipo does not know", async () => {
    inPage({ ...signedIn, member: { ...member, hiddenParts: ["unknown"] } });
    const card = await preview();
    expect(screen.queryByRole("note")).toBeNull();
    expect(within(card).getByText("Pat Lee")).toBeVisible();
  });
});
