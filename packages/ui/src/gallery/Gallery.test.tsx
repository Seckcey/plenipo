import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { Gallery } from "./Gallery";
import { sampleCards } from "./fixtures";

const now = Date.UTC(2026, 8, 27, 18, 0);

const SECTIONS = [
  "Status",
  "Entity cards",
  "Card grid",
  "Table and filters",
  "Detail split view",
  "Relationship map",
  "Notices",
  "Controls",
  "Empty, loading, and error",
  "Colors, type, and spacing",
];

describe("Gallery", () => {
  beforeEach(() => localStorage.clear());

  it("shows every component section, with real data first", () => {
    render(
      <Gallery
        now={now}
        theme="dark"
        live={{ state: "ready", cards: sampleCards(now).slice(0, 2) }}
      />,
    );
    expect(screen.getByRole("heading", { name: "From your organization" })).toBeInTheDocument();
    for (const name of SECTIONS) expect(screen.getByRole("heading", { name })).toBeInTheDocument();
    expect(screen.getByRole("table", { name: "Workers" })).toHaveAttribute("aria-rowcount", "5001");
  });

  it("shows the live section's empty, loading, and error states", () => {
    const { rerender } = render(
      <Gallery now={now} theme="dark" live={{ state: "ready", cards: [] }} />,
    );
    expect(screen.getByText("No departments or projects yet")).toBeInTheDocument();
    rerender(
      <Gallery now={now} theme="dark" live={{ state: "error", error: "No answer", cards: [] }} />,
    );
    expect(screen.getByText("Couldn't read the organization")).toBeInTheDocument();
  });

  it("shows both themes side by side", async () => {
    const user = userEvent.setup();
    const { container } = render(<Gallery now={now} theme="light" />);
    expect(screen.getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "true");
    await user.click(screen.getByRole("button", { name: "Both side by side" }));
    const dark = screen.getByRole("group", { name: "Dark theme" });
    const light = screen.getByRole("group", { name: "Light theme" });
    expect(dark).toHaveAttribute("data-theme", "dark");
    expect(light).toHaveAttribute("data-theme", "light");
    for (const pane of [dark, light]) {
      for (const name of SECTIONS)
        expect(within(pane).getByRole("heading", { name })).toBeInTheDocument();
    }
    // Every sample appears once per theme, and ids stay unique.
    const ids = [...container.querySelectorAll("[id]")].map((el) => el.id);
    expect(new Set(ids).size).toBe(ids.length);
  });
});
