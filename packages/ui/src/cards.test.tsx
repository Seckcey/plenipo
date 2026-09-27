import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import { bucketActivity, DAY_MS } from "./activity";
import { CardGrid, EntityCard, type CollectionView } from "./cards";

const now = Date.UTC(2026, 8, 27, 18, 0);
const series = bucketActivity([{ at: now - 1000 }], now - DAY_MS, now);

describe("EntityCard", () => {
  it("shows the title, status word, subtype, activity, owner, and permissions", async () => {
    const user = userEvent.setup();
    const open = vi.fn();
    render(
      <EntityCard
        title="Website"
        status="pending"
        statusLabel="Waiting for you"
        subtype="Project · Development"
        activity={series}
        now={now}
        owner={{ icon: "user", label: "Supervisor · Codex" }}
        resources={[
          { icon: "globe", label: "Use websites" },
          { icon: "file", label: "Read files" },
        ]}
        onOpen={open}
      />,
    );
    const card = screen.getByRole("article", { name: "Website, Waiting for you" });
    expect(within(card).getByText("Waiting for you")).toBeInTheDocument();
    expect(within(card).getByText("Project · Development")).toBeInTheDocument();
    expect(
      within(card).getByRole("img", { name: /Website activity\. Last 24 hours: 1 event/ }),
    ).toBeInTheDocument();
    expect(within(card).getByText("Now")).toBeInTheDocument();
    expect(within(card).getByText("Supervisor · Codex")).toBeInTheDocument();
    expect(within(card).getByRole("img", { name: "Use websites" })).toBeInTheDocument();
    await user.click(within(card).getByRole("button", { name: "Website" }));
    expect(open).toHaveBeenCalled();
  });

  it("shows empty, loading, and error activity", () => {
    const { rerender } = render(
      <EntityCard title="A" status="offline" statusLabel="Idle" activity={null} />,
    );
    expect(screen.getByText("No activity yet")).toBeInTheDocument();
    expect(screen.getByText("No permissions")).toBeInTheDocument();
    rerender(<EntityCard title="A" status="offline" statusLabel="Idle" activity="loading" />);
    expect(screen.getByText("Loading activity…")).toBeInTheDocument();
    rerender(<EntityCard title="A" status="offline" statusLabel="Idle" activity="error" />);
    expect(screen.getByRole("alert")).toHaveTextContent("Couldn't load activity");
  });
});

interface Item {
  id: string;
  name: string;
}

const many: Item[] = Array.from({ length: 150 }, (_, i) => ({ id: `c${i}`, name: `Card ${i}` }));

function Grid({ items = many, state }: { items?: Item[]; state?: "loading" | "error" }) {
  const [view, setView] = useState<CollectionView>("cards");
  return (
    <CardGrid
      label="Workers"
      items={items}
      getKey={(i) => i.id}
      view={view}
      onViewChange={setView}
      renderCard={(i) => (
        <EntityCard title={i.name} status="ok" statusLabel="Working" onOpen={() => undefined} />
      )}
      renderRow={(i) => <span>{i.name}</span>}
      {...(state ? { state } : {})}
    />
  );
}

describe("CardGrid", () => {
  it("draws only the cards on screen out of 150", () => {
    render(<Grid />);
    const drawn = screen.getAllByRole("article");
    expect(drawn.length).toBeGreaterThan(0);
    expect(drawn.length).toBeLessThan(40);
    expect(screen.getByText("150 items")).toBeInTheDocument();
  });

  it("switches between cards and a list", async () => {
    const user = userEvent.setup();
    render(<Grid items={many.slice(0, 3)} />);
    const cards = screen.getByRole("button", { name: "Cards" });
    const list = screen.getByRole("button", { name: "List" });
    expect(cards).toHaveAttribute("aria-pressed", "true");
    await user.click(list);
    expect(list).toHaveAttribute("aria-pressed", "true");
    expect(screen.queryAllByRole("article")).toHaveLength(0);
    expect(screen.getAllByRole("listitem")).toHaveLength(3);
  });

  it("reaches every card's title from the keyboard", async () => {
    const user = userEvent.setup();
    render(<Grid items={many.slice(0, 3)} />);
    await user.tab();
    expect(screen.getByRole("button", { name: "Cards" })).toHaveFocus();
    await user.tab();
    await user.tab();
    expect(screen.getByRole("button", { name: "Card 0" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Card 1" })).toHaveFocus();
  });

  it("has loading, error, and empty states", () => {
    const { rerender } = render(<Grid state="loading" />);
    expect(screen.getByText("Loading…")).toBeInTheDocument();
    rerender(<Grid state="error" />);
    expect(screen.getByRole("alert")).toHaveTextContent("Couldn't load these");
    rerender(<Grid items={[]} />);
    expect(screen.getByText("Nothing here yet")).toBeInTheDocument();
  });
});
