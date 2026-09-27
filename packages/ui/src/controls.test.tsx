import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import { Checkbox, Segmented, Switch, Tabs } from "./controls";
import { ErrorState, EmptyState, LoadingState } from "./states";

describe("Switch", () => {
  it("is a switch with On and Off in words", async () => {
    const user = userEvent.setup();
    function S() {
      const [on, setOn] = useState(false);
      return <Switch label="Remote computers (SSH)" checked={on} onChange={setOn} />;
    }
    render(<S />);
    const sw = screen.getByRole("switch", { name: "Remote computers (SSH)" });
    expect(sw).toHaveAttribute("aria-checked", "false");
    expect(sw).toHaveTextContent("Off");
    sw.focus();
    await user.keyboard(" ");
    expect(sw).toHaveAttribute("aria-checked", "true");
    expect(sw).toHaveTextContent("On");
  });
});

describe("Checkbox", () => {
  it("shows a count and can be indeterminate", () => {
    render(
      <Checkbox
        label="Online"
        count={7}
        checked={false}
        indeterminate
        onChange={() => undefined}
      />,
    );
    const box = screen.getByRole("checkbox", { name: "Online (7)" });
    expect((box as HTMLInputElement).indeterminate).toBe(true);
  });
});

describe("Segmented", () => {
  it("marks the chosen option as pressed", async () => {
    const user = userEvent.setup();
    function S() {
      const [v, setV] = useState<"a" | "b">("a");
      return (
        <Segmented
          label="View"
          value={v}
          onChange={setV}
          options={[
            { value: "a", label: "Cards" },
            { value: "b", label: "List" },
          ]}
        />
      );
    }
    render(<S />);
    await user.click(screen.getByRole("button", { name: "List" }));
    expect(screen.getByRole("button", { name: "List" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Cards" })).toHaveAttribute("aria-pressed", "false");
  });
});

describe("Tabs", () => {
  it("moves with the arrow keys, Home, and End", async () => {
    const user = userEvent.setup();
    function T() {
      const [v, setV] = useState<"a" | "b" | "c">("a");
      return (
        <Tabs
          label="Terminal"
          value={v}
          onChange={setV}
          idPrefix="t"
          tabs={[
            { value: "a", label: "Your terminal" },
            { value: "b", label: "Senior Developer" },
            { value: "c", label: "Shop server" },
          ]}
        />
      );
    }
    render(<T />);
    await user.tab();
    expect(screen.getByRole("tab", { name: "Your terminal" })).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("tab", { name: "Senior Developer" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("tab", { name: "Senior Developer" })).toHaveFocus();
    await user.keyboard("{End}");
    expect(screen.getByRole("tab", { name: "Shop server" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("tab", { name: "Your terminal" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("tab", { name: "Your terminal" })).toHaveAttribute(
      "aria-controls",
      "t-panel-a",
    );
  });
});

describe("ErrorState", () => {
  it("is polite by default, and interrupts only when urgent", () => {
    const { rerender } = render(<ErrorState title="Couldn't load" />);
    expect(screen.getByRole("status")).toHaveTextContent("Couldn't load");
    expect(screen.queryByRole("alert")).toBeNull();
    rerender(<ErrorState title="Couldn't save" urgent />);
    expect(screen.getByRole("alert")).toHaveTextContent("Couldn't save");
  });
});

describe("states", () => {
  it("empty, loading, and error say what is happening", async () => {
    const user = userEvent.setup();
    const retry = vi.fn();
    render(
      <>
        <EmptyState title="No workers yet">Workers appear here.</EmptyState>
        <LoadingState label="Loading workers" />
        <ErrorState title="Couldn't load workers" message="No answer" onRetry={retry} />
      </>,
    );
    expect(screen.getAllByRole("status")[0]).toHaveTextContent("No workers yet");
    expect(screen.getByText("Loading workers…")).toBeInTheDocument();
    expect(screen.getByText("No answer", { exact: false })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(retry).toHaveBeenCalled();
  });
});
