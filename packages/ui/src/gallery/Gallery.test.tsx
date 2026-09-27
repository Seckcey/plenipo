import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { Gallery } from "./Gallery";
import { sampleCards } from "./fixtures";

const now = Date.UTC(2026, 8, 27, 18, 0);

const SECTIONS = [
  "Status",
  "Cards",
  "Card grid",
  "Table and filters",
  "Detail page",
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
        live={{
          state: "ready",
          cards: sampleCards(now)
            .slice(0, 2)
            .map((c, i) => ({ ...c, id: `card-${i}` })),
        }}
      />,
    );
    expect(screen.getByRole("heading", { name: "From your organization" })).toBeInTheDocument();
    for (const name of SECTIONS) expect(screen.getByRole("heading", { name })).toBeInTheDocument();
    expect(screen.getByRole("table", { name: "Workers" })).toHaveAttribute("aria-rowcount", "5001");
  });

  it("captions every sample in plain words, not code names", () => {
    const { container } = render(<Gallery now={now} theme="dark" />);
    const captions = [...container.querySelectorAll("figcaption")].map((c) => c.textContent ?? "");
    expect(captions.length).toBeGreaterThan(30);
    for (const caption of captions) expect(caption).not.toMatch(/^[a-z]+(-[a-z]+)+$/);
  });

  it("shows the loading, error, and empty states of the filters and the detail page", () => {
    render(<Gallery now={now} theme="dark" />);
    for (const text of [
      "Loading filters…",
      "Couldn't load the filters",
      "No filters here yet",
      "Loading Worker details…",
      "Couldn't load the details",
      "Nothing picked",
      "Loading details…",
      "Nothing to show yet",
      "Loading the timeline…",
      "Couldn't load the timeline",
    ])
      expect(screen.getAllByText(text).length).toBeGreaterThan(0);
    const list = screen.getByRole("region", { name: "Worker list" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(4);
  });

  it("shows the live section's empty, loading, and error states", () => {
    const { rerender } = render(
      <Gallery now={now} theme="dark" live={{ state: "ready", cards: [] }} />,
    );
    expect(screen.getByText("No departments or projects yet")).toBeInTheDocument();
    const { container } = render(
      <Gallery now={now} theme="dark" live={{ state: "loading", cards: [] }} />,
    );
    const live = within(container).getByRole("region", { name: "From your organization" });
    expect(live.querySelectorAll(".ui-card--skeleton")).toHaveLength(2);
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
