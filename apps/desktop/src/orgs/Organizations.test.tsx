import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { OrgFolderInfo, OrgListing, OrgSummary } from "@plenipo/types";
import { storedKey } from "@plenipo/ui";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { OrganizationMenu } from "./OrganizationMenu";
import { OrganizationsSetting } from "./OrganizationsSetting";
import { keyFor, rememberFor } from "./storage";
import { afterChange } from "../test/core";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getOrganizations: vi.fn(),
    getOrganization: vi.fn(),
    createOrganization: vi.fn(),
    switchOrganization: vi.fn(),
    openOrganizationWindow: vi.fn(),
    archiveOrganization: vi.fn(),
    bringBackOrganization: vi.fn(),
    previewDeleteOrganization: vi.fn(),
    deleteOrganizationForGood: vi.fn(),
    renameOrganization: vi.fn(),
    suggestOrgFolder: vi.fn(),
    chooseFolder: vi.fn(),
    getOrgFolder: vi.fn(),
    openOrgFolder: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeOrganizations: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeLicense: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeAgentUpdates: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeRuntimeEvents: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
const user = () => userEvent.setup();

function org(patch: Partial<OrgSummary> & { id: string; name: string }): OrgSummary {
  return {
    first: false,
    archived: false,
    here: false,
    inWindow: false,
    working: 0,
    createdAt: 1,
    ...patch,
  };
}

const CLIENT = "0123456789abcdef0123456789abcdef";
const OLD = "fedcba9876543210fedcba9876543210";

function listing(): OrgListing {
  return {
    current: "first",
    organizations: [
      org({ id: "first", name: "8 West Ventures", first: true, here: true, inWindow: true }),
      org({ id: CLIENT, name: "Client Co" }),
      org({ id: OLD, name: "Old Client", archived: true }),
    ],
    templates: [],
  };
}

const DOCUMENTS = "C:\\Users\\you\\Documents\\Plenipo";

function place(patch: Partial<OrgFolderInfo> = {}): OrgFolderInfo {
  return {
    path: `${DOCUMENTS}\\Acme`,
    exists: false,
    syncedBy: null,
    keptOnThisDevice: null,
    problem: null,
    ...patch,
  };
}

beforeEach(() => {
  localStorage.clear();
  vi.clearAllMocks();
  api.getOrganizations.mockResolvedValue(listing());
  api.suggestOrgFolder.mockImplementation((name, folder) =>
    Promise.resolve(place({ path: `${folder ?? DOCUMENTS}\\${name || "Organization"}` })),
  );
  api.getOrgFolder.mockResolvedValue(place({ path: null }));
  api.openOrgFolder.mockResolvedValue(undefined);
  api.switchOrganization.mockResolvedValue("switching");
  api.openOrganizationWindow.mockResolvedValue("opened");
});

afterEach(() => {
  cleanup();
  rememberFor("first", true);
});

describe("more than one organization (Phase 21, ADR-094)", () => {
  it("each organization remembers its own page, map, and panels in a window", () => {
    expect(keyFor("plenipo.place", "first", true)).toBe("plenipo.place");
    expect(keyFor("plenipo.place", CLIENT, false)).toBe(`plenipo.place@${CLIENT}`);
    expect(keyFor("plenipo.layout", CLIENT, false)).toBe(`plenipo.layout@${CLIENT}`);
    // Your own choices (the theme) are the same in every organization.
    expect(keyFor("plenipo.theme", CLIENT, false)).toBe("plenipo.theme");
    rememberFor(CLIENT, false);
    expect(storedKey("plenipo.orgCamera")).toBe(`plenipo.orgCamera@${CLIENT}`);
    rememberFor("first", true);
    expect(storedKey("plenipo.orgCamera")).toBe("plenipo.orgCamera");
  });

  it("switches this window to another organization, or opens it in a new window", async () => {
    render(<OrganizationMenu go={vi.fn()} />);
    await user().click(await screen.findByRole("button", { name: /Organizations/ }));
    const menu = screen.getByRole("menu");
    // The archived one is not offered; this window's own is not either.
    expect(within(menu).queryByText("Old Client")).toBeNull();
    expect(within(menu).queryByText("8 West Ventures")).toBeNull();
    await user().click(within(menu).getByRole("menuitem", { name: "Switch to Client Co" }));
    expect(api.switchOrganization).toHaveBeenCalledWith(CLIENT);
    await user().click(screen.getByRole("button", { name: /Organizations/ }));
    await user().click(screen.getByRole("menuitem", { name: "Open Client Co in a new window" }));
    expect(api.openOrganizationWindow).toHaveBeenCalledWith(CLIENT);
  });

  it("asks how a new organization starts: a template (later), a copy, or from scratch", async () => {
    api.createOrganization.mockResolvedValue(org({ id: CLIENT, name: "Acme" }));
    render(<OrganizationMenu go={vi.fn()} />);
    await user().click(await screen.findByRole("button", { name: /Organizations/ }));
    await user().click(screen.getByRole("menuitem", { name: /New organization/ }));
    const form = screen.getByRole("form", { name: "New organization" });
    const template = within(form).getByRole("radio", { name: /Use a template/ });
    expect(template).toBeDisabled();
    expect(within(form).getByText("Templates are coming later.")).toBeInTheDocument();
    expect(within(form).getByRole("radio", { name: /Start from scratch/ })).toBeChecked();
    await user().type(within(form).getByRole("textbox", { name: "Name" }), "Acme");
    await user().click(within(form).getByRole("radio", { name: /Copy from one of your/ }));
    const from = within(form).getByRole("combobox", { name: "Copy from" });
    // Only organizations that are not archived can be copied.
    expect(within(from).queryByRole("option", { name: "Old Client" })).toBeNull();
    await user().selectOptions(from, CLIENT);
    await user().click(within(form).getByRole("button", { name: "Create and open" }));
    await waitFor(() =>
      expect(api.createOrganization).toHaveBeenCalledWith(
        "Acme",
        { kind: "copy", from: CLIENT },
        null,
      ),
    );
    // It opens in a new window.
    await waitFor(() => expect(api.openOrganizationWindow).toHaveBeenCalledWith(CLIENT));
  });

  it("renames this organization, archives another, and deletes an archived one for good", async () => {
    afterChange(api.renameOrganization, { name: "8 West IT" } as never, api.getOrganization);
    api.getOrganization.mockResolvedValue({ name: "8 West Ventures" } as never);
    api.archiveOrganization.mockResolvedValue(listing());
    api.previewDeleteOrganization.mockResolvedValue({
      id: OLD,
      name: "Old Client",
      experienced: [
        {
          positionId: "p1",
          title: "Senior Developer",
          roleName: "Senior Developer",
          tasksDone: 12,
          keptLessons: 3,
        },
      ],
      notSaved: [
        { positionId: "p2", title: "Tax Helper", roleName: "Tax", tasksDone: 2, keptLessons: 0 },
      ],
    });
    api.deleteOrganizationForGood.mockResolvedValue(listing());
    render(<OrganizationsSetting />);
    const list = await screen.findByRole("list", { name: "Your organizations" });
    const rows = within(list).getAllByRole("listitem");
    const [first, client, old] = rows as [HTMLElement, HTMLElement, HTMLElement];
    // The first organization stays: it keeps your Workforce and your tile.
    expect(within(first).getByText("Your first organization")).toBeInTheDocument();
    expect(within(first).queryByRole("button", { name: "Archive organization" })).toBeNull();
    await user().click(within(client).getByRole("button", { name: "Archive organization" }));
    expect(api.archiveOrganization).toHaveBeenCalledWith(CLIENT);
    // Renaming this one.
    const name = screen.getByRole("textbox", { name: "This organization's name" });
    await user().clear(name);
    await user().type(name, "8 West IT");
    await user().click(screen.getByRole("button", { name: "Rename" }));
    await waitFor(() => expect(api.renameOrganization).toHaveBeenCalledWith("8 West IT"));
    // Delete for good: asks first, and offers to save its experienced workers.
    await user().click(within(old).getByRole("button", { name: "Delete for good" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete Old Client for good?" });
    expect(within(dialog).getByRole("checkbox", { name: /Senior Developer/ })).toBeChecked();
    expect(within(dialog).getByText(/Tax Helper \(Tax\)/)).toBeInTheDocument();
    const confirm = within(dialog).getByRole("button", { name: "Delete for good" });
    expect(confirm).toBeDisabled();
    await user().click(within(dialog).getByRole("checkbox", { name: /Yes, delete Old Client/ }));
    await user().click(confirm);
    await waitFor(() => expect(api.deleteOrganizationForGood).toHaveBeenCalledWith(OLD, ["p1"]));
  });
});

describe("the organization folder (Phase 25, ADR-205)", () => {
  async function openDialog() {
    render(<OrganizationMenu go={vi.fn()} />);
    await user().click(await screen.findByRole("button", { name: /Organizations/ }));
    await user().click(screen.getByRole("menuitem", { name: /New organization/ }));
    return screen.getByRole("form", { name: "New organization" });
  }

  it("shows where a new organization's folder goes, and lets the owner choose another place", async () => {
    api.createOrganization.mockResolvedValue(org({ id: CLIENT, name: "Acme" }));
    const form = await openDialog();
    await user().type(within(form).getByRole("textbox", { name: "Name" }), "Acme");
    const where = within(form).getByLabelText("Where its folder goes");
    await waitFor(() => expect(where).toHaveTextContent(`${DOCUMENTS}\\Acme`));
    expect(within(form).queryByRole("note", { name: "Keep it on this computer" })).toBeNull();
    // A place that can't be one says why, and nothing changes.
    api.chooseFolder.mockResolvedValueOnce(
      place({ path: "C:\\Windows", problem: "C:\\Windows is one of the system's own folders." }),
    );
    await user().click(within(form).getByRole("button", { name: "Change…" }));
    expect(await within(form).findByRole("alert")).toHaveTextContent("system's own folders");
    // Closing the chooser without choosing changes nothing either.
    api.chooseFolder.mockResolvedValueOnce(null);
    await user().click(within(form).getByRole("button", { name: "Change…" }));
    // A good place: the folder goes there, named after the organization.
    api.chooseFolder.mockResolvedValueOnce(place({ path: "D:\\Work" }));
    await user().click(within(form).getByRole("button", { name: "Change…" }));
    await waitFor(() => expect(where).toHaveTextContent("D:\\Work\\Acme"));
    expect(api.suggestOrgFolder).toHaveBeenLastCalledWith("Acme", "D:\\Work");
    await user().click(within(form).getByRole("button", { name: "Create and open" }));
    await waitFor(() =>
      expect(api.createOrganization).toHaveBeenCalledWith("Acme", { kind: "scratch" }, "D:\\Work"),
    );
  });

  it("a place that can't be used keeps Create closed, with the reason", async () => {
    api.suggestOrgFolder.mockResolvedValue(
      place({ problem: "That's Plenipo's own data folder, where it keeps its records." }),
    );
    const form = await openDialog();
    await user().type(within(form).getByRole("textbox", { name: "Name" }), "Acme");
    expect(await within(form).findByRole("alert")).toHaveTextContent("Plenipo's own data folder");
    expect(within(form).getByRole("button", { name: "Create and open" })).toBeDisabled();
  });

  it("tells the owner to keep a OneDrive folder on this device, in plain words", async () => {
    api.suggestOrgFolder.mockResolvedValue(
      place({
        path: "C:\\Users\\you\\OneDrive\\Documents\\Plenipo\\Acme",
        syncedBy: "oneDrive",
        keptOnThisDevice: false,
      }),
    );
    const form = await openDialog();
    const alert = await within(form).findByRole("note", { name: "Keep it on this computer" });
    expect(alert).toHaveTextContent(
      "Your organization folder is in OneDrive. OneDrive can keep files only online, and your workers can't read those until they download. In File Explorer, right-click the Acme folder and choose Always keep on this device.",
    );
  });

  it("shows this organization's folder in Settings, with the alert until it stays on this device", async () => {
    api.getOrganization.mockResolvedValue({ name: "8 West Ventures" } as never);
    const onedrive = "C:\\Users\\you\\OneDrive\\Documents\\Plenipo\\8 West Ventures";
    api.getOrgFolder
      .mockResolvedValueOnce(
        place({ path: onedrive, exists: true, syncedBy: "oneDrive", keptOnThisDevice: false }),
      )
      .mockResolvedValueOnce(
        place({ path: onedrive, exists: true, syncedBy: "oneDrive", keptOnThisDevice: true }),
      );
    render(<OrganizationsSetting />);
    const section = await screen.findByRole("region", { name: "Organization folder" });
    expect(within(section).getByText(onedrive)).toBeInTheDocument();
    expect(section).toHaveTextContent("OneDrive keeps a copy online.");
    expect(section).toHaveTextContent("Plenipo doesn't copy it.");
    const alert = within(section).getByRole("note", { name: "Keep it on this computer" });
    await user().click(within(alert).getByRole("button", { name: "Show in folder" }));
    expect(api.openOrgFolder).toHaveBeenCalledTimes(1);
    // Set to stay on this device: Check again, and the alert goes.
    await user().click(within(alert).getByRole("button", { name: "Check again" }));
    await waitFor(() =>
      expect(within(section).queryByRole("note", { name: "Keep it on this computer" })).toBeNull(),
    );
  });

  it("another sync service's folder gets its own words, and no folder says so", async () => {
    api.getOrganization.mockResolvedValue({ name: "8 West Ventures" } as never);
    api.getOrgFolder.mockResolvedValueOnce(
      place({ path: "D:\\Dropbox\\Plenipo\\Acme", exists: true, syncedBy: "other" }),
    );
    render(<OrganizationsSetting />);
    const alert = await screen.findByRole("note", { name: "Keep it on this computer" });
    expect(alert).toHaveTextContent("another service syncs online");
    expect(alert).toHaveTextContent("Keep Downloaded");
    expect(within(alert).queryByRole("button", { name: "Check again" })).toBeNull();
    cleanup();
    api.getOrgFolder.mockResolvedValueOnce(place({ path: null }));
    render(<OrganizationsSetting />);
    const section = await screen.findByRole("region", { name: "Organization folder" });
    expect(section).toHaveTextContent("This organization has no organization folder yet.");
    expect(within(section).queryByRole("button", { name: "Show in folder" })).toBeNull();
  });
});
