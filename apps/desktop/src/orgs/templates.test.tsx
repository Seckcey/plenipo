import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { NewDepartmentDialog, NewProjectDialog } from "../components/org/OrgDialogs";
import { sampleOrganization } from "../test/orgFixtures";
import { TemplatesSetting } from "./TemplatesSetting";
import { afterChange } from "../test/core";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getOrganization: vi.fn(),
    applyOrganizationTemplate: vi.fn(),
    saveOrganizationTemplate: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeAgentUpdates: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);

beforeEach(() => {
  api.getOrganization.mockResolvedValue(sampleOrganization());
  afterChange(api.applyOrganizationTemplate, sampleOrganization(), api.getOrganization);
  api.saveOrganizationTemplate.mockResolvedValue({
    current: "first",
    organizations: [],
    templates: [],
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("templates (Phase 25, item 2.8)", () => {
  it("adds a template's departments, with more than one locked on Free, and saves this organization as one", async () => {
    const onSaved = vi.fn();
    render(<TemplatesSetting onFree onSaved={onSaved} />);
    const user = userEvent.setup();
    const pick = await screen.findByRole("combobox", { name: /^Add a template's departments/ });
    const add = screen.getByRole("button", { name: "Add its departments" });
    // Agency adds three departments: part of Pro on Free.
    await user.selectOptions(pick, "agency");
    expect(within(pick).getByRole("option", { name: "Agency (part of Pro)" })).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "What Agency adds" })).toHaveTextContent(
      "Design: Design Manager, with Designer, Researcher",
    );
    expect(add).toBeDisabled();
    // A software project adds one: fine on Free.
    await user.selectOptions(pick, "software");
    expect(add).toBeEnabled();
    await user.click(add);
    expect(api.applyOrganizationTemplate).toHaveBeenCalledWith("software");
    // Saved as a template.
    const form = screen.getByRole("form", { name: "Save this organization as a template" });
    await user.type(within(form).getByRole("textbox"), "My agency setup");
    await user.click(within(form).getByRole("button", { name: "Save as a template" }));
    expect(api.saveOrganizationTemplate).toHaveBeenCalledWith("My agency setup");
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect(form).toHaveTextContent('Saved "My agency setup"');
  });

  it("offers department templates the organization doesn't have yet, and the Software project for a project", async () => {
    const onTemplate = vi.fn().mockResolvedValue(null);
    const s = sampleOrganization();
    const { unmount } = render(
      <NewDepartmentDialog
        snapshot={s}
        onCancel={() => undefined}
        onSubmit={vi.fn()}
        onTemplate={onTemplate}
      />,
    );
    const user = userEvent.setup();
    const pick = screen.getByRole("combobox", { name: /^Template/ });
    expect(
      within(pick)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual([
      "Operations: Keeps the servers, computers, and accounts running and safe.",
      "Design: Graphics, layouts, and the brand.",
    ]);
    await user.click(screen.getByRole("button", { name: "Add Operations" }));
    expect(onTemplate).toHaveBeenCalledWith("operations");
    unmount();
    const software = vi.fn();
    render(
      <NewProjectDialog
        snapshot={s}
        onCancel={() => undefined}
        onSubmit={vi.fn()}
        onTemplate={software}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Use the Software project template" }));
    expect(software).toHaveBeenCalled();
  });
});
