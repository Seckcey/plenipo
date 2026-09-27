import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import {
  EMPTY_FACETS,
  facetsActive,
  filterItems,
  useFacets,
  type FacetConfig,
} from "./facet-logic";
import { FacetPanel } from "./facets";

interface Item {
  name: string;
  status: string;
  tool: string;
  age: number;
}

const items: Item[] = [
  { name: "Senior Developer", status: "ok", tool: "Codex", age: 1 },
  { name: "Code Reviewer", status: "ok", tool: "Claude Code", age: 30 },
  { name: "QA Engineer", status: "warn", tool: "Codex", age: 200 },
  { name: "Designer", status: "offline", tool: "Grok", age: 800 },
];

const config: FacetConfig<Item> = {
  search: (i) => i.name,
  groups: [
    {
      id: "status",
      label: "Status",
      value: (i) => i.status,
      labels: { ok: "Working", warn: "Blocked", offline: "Idle" },
    },
    { id: "tool", label: "AI tool", value: (i) => i.tool },
  ],
  ranges: [{ id: "age", label: "Last seen", min: 0, max: 1000, value: (i) => i.age }],
};

describe("filterItems", () => {
  it("applies search, checkboxes, and ranges together", () => {
    expect(
      filterItems(items, { ...EMPTY_FACETS, search: "code" }, config).map((i) => i.name),
    ).toEqual(["Code Reviewer"]);
    expect(
      filterItems(items, { ...EMPTY_FACETS, checks: { tool: ["Codex"] } }, config).map(
        (i) => i.name,
      ),
    ).toEqual(["Senior Developer", "QA Engineer"]);
    expect(
      filterItems(items, { ...EMPTY_FACETS, ranges: { age: [0, 100] } }, config).map((i) => i.name),
    ).toEqual(["Senior Developer", "Code Reviewer"]);
  });

  it("knows when any filter is on", () => {
    expect(facetsActive(EMPTY_FACETS, config.ranges)).toBe(false);
    expect(facetsActive({ ...EMPTY_FACETS, ranges: { age: [0, 1000] } }, config.ranges)).toBe(
      false,
    );
    expect(facetsActive({ ...EMPTY_FACETS, ranges: { age: [5, 1000] } }, config.ranges)).toBe(true);
  });
});

function Bound() {
  const f = useFacets(items, config);
  return (
    <>
      <FacetPanel
        state={f.state}
        onChange={f.setState}
        groups={f.groups}
        ranges={f.ranges}
        collapsed={false}
        onCollapsedChange={() => undefined}
        total={items.length}
        shown={f.filtered.length}
      />
      <ul aria-label="Results">
        {f.filtered.map((i) => (
          <li key={i.name}>{i.name}</li>
        ))}
      </ul>
    </>
  );
}

describe("FacetPanel bound to a list", () => {
  it("filters with checkboxes whose counts ignore their own group", async () => {
    const user = userEvent.setup();
    render(<Bound />);
    const results = screen.getByRole("list", { name: "Results" });
    expect(within(results).getAllByRole("listitem")).toHaveLength(4);
    await user.click(screen.getByRole("checkbox", { name: "Codex (2)" }));
    expect(within(results).getAllByRole("listitem")).toHaveLength(2);
    expect(screen.getByRole("status")).toHaveTextContent("Showing 2 of 4");
    // The tool group still counts every tool; Status counts only Codex's workers now.
    expect(screen.getByRole("checkbox", { name: "Grok (1)" })).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Idle (0)" })).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Working (1)" })).toBeInTheDocument();
  });

  it("searches, and Clear filters brings everything back", async () => {
    const user = userEvent.setup();
    render(<Bound />);
    const clear = screen.getByRole("button", { name: "Clear filters" });
    expect(clear).toBeDisabled();
    await user.type(screen.getByRole("searchbox", { name: "Search" }), "design");
    expect(
      within(screen.getByRole("list", { name: "Results" })).getAllByRole("listitem"),
    ).toHaveLength(1);
    expect(clear).toBeEnabled();
    await user.click(clear);
    expect(
      within(screen.getByRole("list", { name: "Results" })).getAllByRole("listitem"),
    ).toHaveLength(4);
    expect(screen.getByRole("searchbox", { name: "Search" })).toHaveValue("");
  });

  it("has a two-handle range slider whose handles cannot cross", () => {
    render(<Bound />);
    const low = screen.getByRole("slider", { name: "Last seen, lowest" });
    const high = screen.getByRole("slider", { name: "Last seen, highest" });
    expect(low).toHaveValue("0");
    expect(high).toHaveValue("1000");
    fireEvent.change(high, { target: { value: "50" } });
    expect(
      within(screen.getByRole("list", { name: "Results" })).getAllByRole("listitem"),
    ).toHaveLength(2);
    // The lowest handle cannot pass the highest.
    fireEvent.change(low, { target: { value: "900" } });
    expect(low).toHaveValue("50");
    expect(high).toHaveAttribute("aria-valuetext", "50");
  });

  it("collapses to a strip and opens again", async () => {
    const user = userEvent.setup();
    const onCollapsed = vi.fn();
    const { rerender } = render(
      <FacetPanel
        state={EMPTY_FACETS}
        onChange={() => undefined}
        groups={[]}
        collapsed={false}
        onCollapsedChange={onCollapsed}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Hide filters" }));
    expect(onCollapsed).toHaveBeenCalledWith(true);
    rerender(
      <FacetPanel
        state={EMPTY_FACETS}
        onChange={() => undefined}
        groups={[]}
        collapsed
        onCollapsedChange={onCollapsed}
      />,
    );
    expect(screen.queryByRole("searchbox")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Show filters" }));
    expect(onCollapsed).toHaveBeenCalledWith(false);
  });

  it("folds a group away and back", async () => {
    const user = userEvent.setup();
    render(<Bound />);
    const head = screen.getByRole("button", { name: "AI tool" });
    expect(head).toHaveAttribute("aria-expanded", "true");
    await user.click(head);
    expect(head).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("checkbox", { name: /Codex/ })).toBeNull();
  });
});
