import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { Hero, PageHeader, Panel, RowList, StatGrid } from "./page";
import { EmptyState } from "./states";

describe("page parts", () => {
  it("heads a page with its kind, name, line, buttons, and Back", async () => {
    const user = userEvent.setup();
    const back = vi.fn();
    render(
      <PageHeader
        kicker="Department"
        title="Operations"
        lead="Keeps things running."
        onBack={back}
        id="h"
      />,
    );
    expect(screen.getByRole("heading", { level: 1, name: "Operations" })).toHaveAttribute(
      "id",
      "h",
    );
    expect(screen.getByText("Department")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(back).toHaveBeenCalledTimes(1);
  });

  it("titles a panel, with a count spoken in words", () => {
    render(
      <Panel title="Waiting for you" count={2} countLabel="waiting" id="w">
        body
      </Panel>,
    );
    const panel = screen.getByRole("region", { name: "Waiting for you" });
    expect(within(panel).getByText("body")).toBeInTheDocument();
    expect(within(panel).getByLabelText("2 waiting")).toBeInTheDocument();
  });

  it("lists rows to open, with their status in words, and says when there are none", async () => {
    const user = userEvent.setup();
    const open = vi.fn();
    const { rerender } = render(
      <RowList
        label="Objectives"
        items={[
          {
            id: "1",
            title: "Order stock",
            detail: "Shop",
            status: { status: "ok", label: "Working" },
            meta: "5 min",
            onOpen: open,
          },
          { id: "2", title: "Not openable" },
        ]}
      />,
    );
    const list = screen.getByRole("list", { name: "Objectives" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
    await user.click(within(list).getByRole("button", { name: /Order stock/ }));
    expect(open).toHaveBeenCalled();
    expect(within(list).getByText("Working")).toBeInTheDocument();
    expect(within(list).queryAllByRole("button")).toHaveLength(1);
    rerender(<RowList label="Objectives" items={[]} empty={<EmptyState title="None yet" />} />);
    expect(screen.getByRole("status")).toHaveTextContent("None yet");
    rerender(
      <RowList
        label="objectives"
        items={[]}
        state="error"
        error="No answer"
        onRetry={() => undefined}
      />,
    );
    expect(screen.getByText("Couldn't load objectives")).toBeInTheDocument();
  });

  it("shows numbers as tiles, and opens what a tile counts", async () => {
    const user = userEvent.setup();
    const open = vi.fn();
    render(
      <StatGrid
        label="Today"
        stats={[
          { label: "Working", value: 3, status: "ok" },
          { label: "Waiting for you", value: 1, onOpen: open },
        ]}
      />,
    );
    expect(screen.getByText("Working").nextSibling).toHaveTextContent("3");
    await user.click(screen.getByRole("button", { name: /Waiting for you/ }));
    expect(open).toHaveBeenCalled();
  });

  it("greets the owner with Pip", () => {
    const { container } = render(
      <Hero pip="welcome" title="Good morning" id="g">
        All quiet.
      </Hero>,
    );
    expect(screen.getByRole("region", { name: "Good morning" })).toHaveTextContent("All quiet.");
    expect(container.querySelector("[data-pip=welcome]")).not.toBeNull();
  });
});
