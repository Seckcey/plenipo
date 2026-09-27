import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import { DAY_MS } from "./activity";
import { DetailSplitView, PropertyList, TimelineScrubber, type TimelineValue } from "./detail";
import { TopologyMap } from "./topology";
import { layoutMap, type MapLink, type MapNode } from "./topology-layout";

const now = Date.UTC(2026, 8, 27, 18, 0);

function Scrubber() {
  const [value, setValue] = useState<TimelineValue>("live");
  return (
    <TimelineScrubber
      from={now - DAY_MS}
      to={now}
      events={[
        { at: now - 3_600_000, label: "Push", status: "pending" },
        { at: now - 2 * DAY_MS, label: "Too old" },
      ]}
      value={value}
      onChange={setValue}
    />
  );
}

describe("TimelineScrubber", () => {
  it("starts live and moves with the keyboard", async () => {
    const user = userEvent.setup();
    render(<Scrubber />);
    expect(screen.getByText("(1 event)")).toBeInTheDocument();
    const thumb = screen.getByRole("slider", { name: "Timeline" });
    expect(thumb).toHaveAttribute("aria-valuetext", "Live");
    thumb.focus();
    await user.keyboard("{ArrowDown}");
    expect(thumb).toHaveAttribute("aria-valuenow", String(now - 15 * 60_000));
    await user.keyboard("{PageDown}");
    expect(thumb).toHaveAttribute("aria-valuenow", String(now - 75 * 60_000));
    expect(screen.getByRole("button", { name: "Back to live" })).toBeInTheDocument();
    await user.keyboard("{Home}");
    expect(thumb).toHaveAttribute("aria-valuenow", String(now - DAY_MS));
    await user.keyboard("{End}");
    expect(thumb).toHaveAttribute("aria-valuetext", "Live");
    expect(screen.queryByRole("button", { name: "Back to live" })).toBeNull();
  });
});

const nodes: MapNode[] = [
  { id: "a", label: "VP", status: "ok", statusLabel: "Working" },
  { id: "b", label: "Manager", status: "pending", statusLabel: "Waiting", caption: "3 tasks" },
  { id: "c", label: "Developer", status: "error", statusLabel: "Failed" },
  { id: "d", label: "Reviewer", status: "offline", statusLabel: "Idle" },
];
const links: MapLink[] = [
  { from: "a", to: "b", label: "objective" },
  { from: "b", to: "c", label: "build" },
  { from: "b", to: "d", label: "review" },
];

describe("TopologyMap", () => {
  it("places parents above and centered over their children, without overlap", () => {
    const { placed } = layoutMap(nodes, links);
    const [a, b, c, d] = ["a", "b", "c", "d"].map((id) => placed.get(id));
    expect(a && b && c && d).toBeTruthy();
    if (!a || !b || !c || !d) return;
    expect(a.y).toBeLessThan(b.y);
    expect(b.y).toBeLessThan(c.y);
    expect(c.y).toBe(d.y);
    expect(b.x).toBe((c.x + d.x) / 2);
    expect(Math.abs(c.x - d.x)).toBeGreaterThanOrEqual(132);
  });

  it("survives a cycle", () => {
    const { placed } = layoutMap(nodes.slice(0, 2), [
      { from: "a", to: "b" },
      { from: "b", to: "a" },
    ]);
    expect(placed.size).toBe(2);
  });

  it("labels each tile with its status in words, and lists the connections", async () => {
    const user = userEvent.setup();
    const select = vi.fn();
    render(
      <TopologyMap
        label="Delegation tree"
        nodes={nodes}
        links={links}
        selectedId="b"
        onSelect={select}
      />,
    );
    const map = screen.getByRole("group", { name: "Delegation tree" });
    expect(within(map).getByRole("button", { name: "Manager, Waiting, 3 tasks" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(within(map).getByText("Failed")).toBeInTheDocument();
    expect(within(map).getByRole("list", { name: "Connections" })).toHaveTextContent(
      "Manager to Developer: build",
    );
    await user.click(within(map).getByRole("button", { name: /^Developer/ }));
    expect(select).toHaveBeenCalledWith("c");
  });

  it("has empty, loading, and error states", () => {
    const { rerender } = render(<TopologyMap label="Map" nodes={[]} links={[]} />);
    expect(screen.getByText("Nothing to map yet")).toBeInTheDocument();
    rerender(<TopologyMap label="Map" nodes={[]} links={[]} state="loading" />);
    expect(screen.getByText("Loading map…")).toBeInTheDocument();
    rerender(<TopologyMap label="Map" nodes={[]} links={[]} state="error" error="No answer" />);
    expect(screen.getByRole("alert")).toHaveTextContent("Couldn't load map");
  });
});

describe("DetailSplitView", () => {
  it("lays out properties, timeline, map, and table", () => {
    render(
      <DetailSplitView
        label="Website Supervisor"
        properties={<PropertyList title="Details" items={[{ label: "AI tool", value: "Codex" }]} />}
        timeline={<Scrubber />}
        map={<TopologyMap label="Tree" nodes={nodes} links={links} />}
        table={<table aria-label="Team" />}
      />,
    );
    const view = screen.getByRole("region", { name: "Website Supervisor" });
    expect(within(view).getByText("AI tool").nextSibling).toHaveTextContent("Codex");
    expect(within(view).getByRole("slider")).toBeInTheDocument();
    expect(within(view).getByRole("group", { name: "Tree" })).toBeInTheDocument();
    expect(within(view).getByRole("table", { name: "Team" })).toBeInTheDocument();
  });
});
