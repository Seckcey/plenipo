import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SYSTEM_WORDS } from "@plenipo/types";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { setSystemWords } from "../system/words";
import { DeletePlenipoData } from "./DeletePlenipoData";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return { ...actual, deletePlenipoData: vi.fn() };
});
const api = vi.mocked(commands);

afterEach(() => {
  setSystemWords(SYSTEM_WORDS.windows);
  vi.clearAllMocks();
});

describe("Delete my Plenipo data (Phase 23)", () => {
  it("is not on Windows, whose uninstaller has its own tick box", () => {
    const { container } = render(<DeletePlenipoData />);
    expect(container).toBeEmptyDOMElement();
  });

  it("on Linux, asks first, then asks again when work is running", async () => {
    setSystemWords(SYSTEM_WORDS.linux);
    api.deletePlenipoData
      .mockRejectedValueOnce(
        new commands.PlenipoCommandError(
          "invalidInput",
          "Work is running. Deleting your data stops it; say so to go ahead.",
        ),
      )
      .mockResolvedValueOnce(undefined);
    render(<DeletePlenipoData />);
    expect(screen.getByText(/your computer's password store/)).toBeInTheDocument();
    expect(screen.getByText(/remove it with your software manager/)).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Delete my Plenipo data" }));
    expect(api.deletePlenipoData).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Delete everything and quit" }));
    expect(api.deletePlenipoData).toHaveBeenCalledWith(false);
    await user.click(
      await screen.findByRole("button", { name: "Stop the work, delete, and quit" }),
    );
    expect(api.deletePlenipoData).toHaveBeenLastCalledWith(true);
  });

  it("on a Mac, says to drag Plenipo to the Trash afterwards", () => {
    setSystemWords(SYSTEM_WORDS.mac);
    render(<DeletePlenipoData />);
    expect(screen.getByText(/drag Plenipo to the Trash/)).toBeInTheDocument();
    expect(screen.getByText(/your Mac's Keychain/)).toBeInTheDocument();
  });
});
