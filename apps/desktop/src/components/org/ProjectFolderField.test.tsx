import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as commands from "../../api/commands";
import { a11yProblems } from "../../test/a11y";
import { ProjectFolderField } from "./ProjectFolderField";

vi.mock("../../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return { ...actual, getOrgFolder: vi.fn(), chooseFolder: vi.fn() };
});

const api = vi.mocked(commands);

const ORG = {
  path: "C:\\Users\\you\\Documents\\Plenipo\\Acme",
  exists: true,
  syncedBy: null,
  keptOnThisDevice: null,
  problem: null,
};

/** The field in a form, keeping what it is given, as the project dialogs do. */
function Harness({ start = "", seen }: { start?: string; seen: (path: string) => void }) {
  const [value, setValue] = useState(start);
  return (
    <main>
      <h1>New project</h1>
      <form aria-label="Project">
        <ProjectFolderField
          value={value}
          onChange={(path) => {
            setValue(path);
            seen(path);
          }}
        />
      </form>
    </main>
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Where a project's files go (ADR-205 §2.4)", () => {
  it("is the folder box as before when the organization has no organization folder", async () => {
    api.getOrgFolder.mockRejectedValue(new Error("no folder"));
    render(<Harness seen={vi.fn()} />);
    expect(await screen.findByLabelText(/^Project folder \(optional\)/)).toBeInTheDocument();
    expect(screen.queryByRole("radio")).toBeNull();
  });

  it("makes a folder in the organization folder by default, and takes one you already have", async () => {
    api.getOrgFolder.mockResolvedValue(ORG);
    const seen = vi.fn();
    const { container } = render(<Harness seen={seen} />);
    const inOrg = await screen.findByRole("radio", {
      name: "Make a folder in the organization folder",
    });
    expect(inOrg).toBeChecked();
    expect(screen.getByText(/Its workers save their work there/)).toBeInTheDocument();
    expect(screen.queryByLabelText(/^Project folder/)).toBeNull();
    const user = userEvent.setup();
    await user.click(screen.getByRole("radio", { name: "Use a folder I already have" }));
    // Typed, or picked with the system's own folder chooser.
    await user.type(screen.getByLabelText(/^Project folder/), "D:\\Work\\Site");
    expect(seen).toHaveBeenLastCalledWith("D:\\Work\\Site");
    api.chooseFolder.mockResolvedValue({ ...ORG, path: "D:\\Work\\Chosen" });
    await user.click(screen.getByRole("button", { name: "Choose…" }));
    await waitFor(() => expect(seen).toHaveBeenLastCalledWith("D:\\Work\\Chosen"));
    expect(screen.getByLabelText(/^Project folder/)).toHaveValue("D:\\Work\\Chosen");
    expect(a11yProblems(container)).toEqual([]);
    // Back to the organization folder: the folder is cleared.
    await user.click(inOrg);
    expect(seen).toHaveBeenLastCalledWith("");
  });

  it("says why a chosen folder can't be used, and keeps what was there", async () => {
    api.getOrgFolder.mockResolvedValue(ORG);
    api.chooseFolder.mockResolvedValue({
      ...ORG,
      path: "C:\\",
      problem:
        "That's the top of a drive. Choose a folder inside it, such as a folder in Documents.",
    });
    const seen = vi.fn();
    render(<Harness start={"D:\\Work\\Site"} seen={seen} />);
    // A project that has a folder starts on it.
    expect(await screen.findByRole("radio", { name: "Use a folder I already have" })).toBeChecked();
    await userEvent.setup().click(screen.getByRole("button", { name: "Choose…" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("That's the top of a drive.");
    expect(seen).not.toHaveBeenCalled();
    expect(screen.getByLabelText(/^Project folder/)).toHaveValue("D:\\Work\\Site");
  });
});
