import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { OrgListing, OrgSummary } from "@plenipo/types";
import { storedKey } from "@plenipo/ui";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { OrganizationMenu } from "./OrganizationMenu";
import { OrganizationsSetting } from "./OrganizationsSetting";
import { keyFor, rememberFor } from "./storage";

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
  };
});
vi.mock("../api/events", () => ({
  subscribeOrganizations: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
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

beforeEach(() => {
  localStorage.clear();
  vi.clearAllMocks();
  api.getOrganizations.mockResolvedValue(listing());
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
      expect(api.createOrganization).toHaveBeenCalledWith("Acme", { kind: "copy", from: CLIENT }),
    );
    // It opens in a new window.
    await waitFor(() => expect(api.openOrganizationWindow).toHaveBeenCalledWith(CLIENT));
  });

  it("renames this organization, archives another, and deletes an archived one for good", async () => {
    api.renameOrganization.mockResolvedValue({ name: "8 West IT" } as never);
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
