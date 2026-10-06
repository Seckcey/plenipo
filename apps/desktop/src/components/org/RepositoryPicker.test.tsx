import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import type { GithubRepositories } from "@plenipo/types";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as commands from "../../api/commands";
import { a11yProblems } from "../../test/a11y";
import { RepositoryPicker } from "./RepositoryPicker";
import { addressOf, matching, updatedWords } from "./repositoryWords";

vi.mock("../../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return { ...actual, listGithubRepositories: vi.fn(), openGithubPage: vi.fn() };
});

const api = vi.mocked(commands);

const LIST: GithubRepositories = {
  accounts: [
    { login: "frankieg", organization: false, allRepositories: true },
    { login: "8west", organization: true, allRepositories: true },
  ],
  repositories: [
    { owner: "frankieg", name: "notes", private: false, description: "My notes" },
    {
      owner: "8west",
      name: "plenipo",
      private: true,
      description: "The app",
      updatedAt: new Date(Date.now() - 2 * 86_400_000).toISOString(),
    },
    { owner: "8west", name: "website", private: false },
  ],
  more: false,
  installPage: "https://github.com/apps/plenipo-by-8-west-ventures/installations/new",
};

/** The box in a form that keeps what it is given, as the project dialogs do. */
function Harness({ seen }: { seen: (address: string) => void }) {
  const [value, setValue] = useState("");
  return (
    <main>
      <h1>New project</h1>
      <form aria-label="Project">
        <RepositoryPicker
          value={value}
          onChange={(address) => {
            setValue(address);
            seen(address);
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

describe("The repository box (ADR-204)", () => {
  it("is the plain box, saying where to connect GitHub, when it isn't connected", async () => {
    api.listGithubRepositories.mockRejectedValue({
      kind: "invalidInput",
      message: "GitHub isn't connected. Settings → Connections → GitHub.",
    });
    const seen = vi.fn();
    render(<Harness seen={seen} />);
    expect(await screen.findByText(/Connect GitHub in Settings → Connections/)).toBeInTheDocument();
    const box = screen.getByLabelText(/^Repository URL \(optional\)/);
    expect(box).not.toHaveAttribute("role", "combobox");
    // Any address, from any host, still works.
    await userEvent.setup().type(box, "https://gitlab.com/acme/site");
    expect(seen).toHaveBeenLastCalledWith("https://gitlab.com/acme/site");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("searches your repositories, grouped by account, and picks one with the keyboard or a click", async () => {
    api.listGithubRepositories.mockResolvedValue(LIST);
    const seen = vi.fn();
    const { container } = render(<Harness seen={seen} />);
    expect(await screen.findByText(/3 repositories from GitHub/)).toBeInTheDocument();
    expect(api.listGithubRepositories).toHaveBeenCalledWith(false);
    const box = screen.getByRole("combobox", { name: /^Repository URL \(optional\)/ });
    const user = userEvent.setup();
    await user.click(box);
    const list = screen.getByRole("listbox", { name: "Your GitHub repositories" });
    expect(within(list).getByRole("group", { name: "frankieg" })).toBeInTheDocument();
    const org = within(list).getByRole("group", { name: "8west" });
    expect(within(org).getAllByRole("option")).toHaveLength(2);
    const app = within(org).getByRole("option", { name: /8west\/plenipo/ });
    expect(app).toHaveTextContent("Private");
    expect(app).toHaveTextContent("The app");
    expect(app).toHaveTextContent("updated 2 days ago");
    expect(a11yProblems(container)).toEqual([]);
    // Typing narrows it; arrow keys and Enter pick, and the address is built from the name.
    await user.type(box, "web");
    expect(screen.getAllByRole("option")).toHaveLength(1);
    await user.keyboard("{ArrowDown}");
    expect(box).toHaveAttribute(
      "aria-activedescendant",
      screen.getByRole("option", { name: /8west\/website/ }).id,
    );
    await user.keyboard("{Enter}");
    expect(seen).toHaveBeenLastCalledWith("https://github.com/8west/website");
    expect(screen.queryByRole("listbox")).toBeNull();
    // A click picks too; Escape closes the list without picking.
    await user.clear(box);
    await user.click(screen.getByRole("option", { name: /frankieg\/notes/ }));
    expect(seen).toHaveBeenLastCalledWith("https://github.com/frankieg/notes");
    await user.type(box, "x");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("adds an account on GitHub's page, and looks again", async () => {
    api.listGithubRepositories.mockResolvedValue(LIST);
    api.openGithubPage.mockResolvedValue(undefined);
    render(<Harness seen={vi.fn()} />);
    await screen.findByText(/3 repositories from GitHub/);
    const user = userEvent.setup();
    await user.click(screen.getByRole("combobox"));
    await user.click(
      screen.getByRole("button", { name: "Not here? Add an account or organization on GitHub" }),
    );
    expect(api.openGithubPage).toHaveBeenCalledWith("install");
    await user.click(screen.getByRole("button", { name: "Look again" }));
    expect(api.listGithubRepositories).toHaveBeenLastCalledWith(true);
  });

  it("says when GitHub refuses, and the box still takes an address", async () => {
    api.listGithubRepositories.mockRejectedValueOnce({
      kind: "invalidInput",
      message: "GitHub didn't allow this. Sign in with GitHub again.",
    });
    const seen = vi.fn();
    render(<Harness seen={seen} />);
    expect(
      await screen.findByText(/GitHub didn't allow this. Sign in with GitHub again./),
    ).toBeInTheDocument();
    const user = userEvent.setup();
    await user.type(screen.getByLabelText(/^Repository URL/), "https://github.com/a/b");
    expect(seen).toHaveBeenLastCalledWith("https://github.com/a/b");
    api.listGithubRepositories.mockResolvedValueOnce(LIST);
    await user.click(screen.getByRole("button", { name: "Look again" }));
    expect(await screen.findByText(/3 repositories from GitHub/)).toBeInTheDocument();
  });
});

describe("The repository box's words", () => {
  it("builds the address from the owner and name", () => {
    expect(addressOf({ owner: "8west", name: "plenipo", private: true })).toBe(
      "https://github.com/8west/plenipo",
    );
  });

  it("matches by name, owner, or description, and a pasted GitHub address", () => {
    const all = LIST.repositories;
    expect(matching(all, "").map((r) => r.name)).toEqual(["notes", "plenipo", "website"]);
    expect(matching(all, "8west").map((r) => r.name)).toEqual(["plenipo", "website"]);
    expect(matching(all, "app").map((r) => r.name)).toEqual(["plenipo"]);
    expect(matching(all, "https://github.com/8west/website.git").map((r) => r.name)).toEqual([
      "website",
    ]);
  });

  it("says when it was updated", () => {
    const now = Date.parse("2026-10-05T12:00:00Z");
    expect(updatedWords(undefined, now)).toBeNull();
    expect(updatedWords("2026-10-05T08:00:00Z", now)).toBe("updated today");
    expect(updatedWords("2026-10-04T08:00:00Z", now)).toBe("updated yesterday");
    expect(updatedWords("2026-09-25T12:00:00Z", now)).toBe("updated 10 days ago");
    expect(updatedWords("2026-04-05T12:00:00Z", now)).toBe("updated 6 months ago");
  });
});
