import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import { CellLink, DataTable } from "./table";
import { sortRows, statusColumn, type Column } from "./table-columns";

interface Row {
  id: string;
  name: string;
  tasks: number;
  tool: string;
  project: string;
  ok: boolean;
}

const rows: Row[] = Array.from({ length: 5000 }, (_, i) => ({
  id: `r${i}`,
  name: `Worker ${String(i).padStart(4, "0")}`,
  tasks: (i * 7919) % 5000,
  tool: i % 2 ? "Codex" : "Claude Code",
  project: i % 3 ? "Website" : "Shop",
  ok: i % 5 !== 0,
}));

const opened: string[] = [];
const columns: Column<Row>[] = [
  { id: "name", header: "Name", cell: (r) => r.name, sortValue: (r) => r.name },
  statusColumn((r) =>
    r.ok ? { status: "ok", label: "Working" } : { status: "error", label: "Failed" },
  ),
  { id: "tool", header: "AI tool", cell: (r) => r.tool, sortValue: (r) => r.tool },
  {
    id: "project",
    header: "Project",
    cell: (r) => <CellLink onClick={() => opened.push(r.project)}>{r.project}</CellLink>,
  },
  { id: "tasks", header: "Tasks", numeric: true, cell: (r) => r.tasks, sortValue: (r) => r.tasks },
];

function Table(props: { rows?: Row[]; pageSize?: number | "all"; storageKey?: string }) {
  const [selected, setSelected] = useState<Set<string>>(new Set());
  return (
    <DataTable
      label="Workers"
      rows={props.rows ?? rows}
      columns={columns}
      getRowId={(r) => r.id}
      selectable
      selected={selected}
      onSelectedChange={setSelected}
      defaultPageSize={props.pageSize ?? "all"}
      {...(props.storageKey ? { storageKey: props.storageKey } : {})}
    />
  );
}

const bodyRows = () =>
  [...document.querySelectorAll("tbody tr")].filter(
    (tr) => !tr.classList.contains("ui-table__spacer"),
  );

beforeEach(() => localStorage.clear());

describe("5,000 rows", () => {
  it("renders quickly and draws only the rows on screen", () => {
    const started = performance.now();
    render(<Table />);
    const elapsed = performance.now() - started;
    expect(bodyRows().length).toBeGreaterThan(10);
    expect(bodyRows().length).toBeLessThan(80);
    expect(screen.getByRole("table", { name: "Workers" })).toHaveAttribute("aria-rowcount", "5001");
    expect(screen.getByText("1–5,000 of 5,000 records")).toBeInTheDocument();
    // Generous for slow CI runners; a full render of 5,000 rows takes far longer.
    expect(elapsed).toBeLessThan(3000);
  });

  it("draws later rows when scrolled", async () => {
    render(<Table />);
    const scroller = document.querySelector(".ui-table__scroll") as HTMLElement;
    await act(async () => {
      scroller.scrollTop = 30 + 4000 * 28;
      fireEvent.scroll(scroller);
      await new Promise((r) => setTimeout(r, 50));
    });
    expect(screen.getByText("Worker 4000")).toBeInTheDocument();
    expect(screen.queryByText("Worker 0000")).toBeNull();
    expect(bodyRows().length).toBeLessThan(80);
  });

  it("sorts 5,000 rows in both directions", async () => {
    const user = userEvent.setup();
    render(<Table />);
    await user.click(screen.getByRole("button", { name: "Tasks" }));
    const header = screen.getByRole("columnheader", { name: "Tasks" });
    expect(header).toHaveAttribute("aria-sort", "ascending");
    expect(within(bodyRows()[0] as HTMLElement).getByText("0")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Tasks" }));
    expect(header).toHaveAttribute("aria-sort", "descending");
    expect(within(bodyRows()[0] as HTMLElement).getByText("4999")).toBeInTheDocument();
  });
});

describe("pages", () => {
  it("counts records and moves between pages", async () => {
    const user = userEvent.setup();
    render(<Table pageSize={100} />);
    expect(screen.getByText("1–100 of 5,000 records")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Previous page" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Next page" }));
    expect(screen.getByText("101–200 of 5,000 records")).toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText("Rows per page"), "25");
    expect(screen.getByText("1–25 of 5,000 records")).toBeInTheDocument();
  });
});

describe("selection", () => {
  it("selects one row, then all 5,000", async () => {
    const user = userEvent.setup();
    render(<Table />);
    await user.click(screen.getByRole("checkbox", { name: "Select row 1" }));
    expect(screen.getByText("1 selected")).toBeInTheDocument();
    expect(bodyRows()[0]).toHaveAttribute("aria-selected", "true");
    const all = screen.getByRole("checkbox", { name: "Select all 5,000" });
    expect((all as HTMLInputElement).indeterminate).toBe(true);
    await user.click(all);
    expect(screen.getByText("5,000 selected")).toBeInTheDocument();
  });
});

describe("columns", () => {
  it("hides a column and remembers it", async () => {
    const user = userEvent.setup();
    const { unmount } = render(<Table storageKey="test.table" />);
    await user.click(screen.getByRole("button", { name: "Columns" }));
    const picker = screen.getByRole("group", { name: "Choose columns" });
    expect(within(picker).getByRole("checkbox", { name: "Name" })).toBeDisabled();
    await user.click(within(picker).getByRole("checkbox", { name: "AI tool" }));
    expect(screen.queryByRole("columnheader", { name: "AI tool" })).toBeNull();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("group", { name: "Choose columns" })).toBeNull();
    unmount();
    render(<Table storageKey="test.table" />);
    expect(screen.queryByRole("columnheader", { name: "AI tool" })).toBeNull();
    expect(screen.getByRole("columnheader", { name: "Tasks" })).toBeInTheDocument();
  });

  it("links to parent entities from a cell", async () => {
    const user = userEvent.setup();
    render(<Table rows={rows.slice(0, 3)} />);
    await user.click(screen.getAllByRole("button", { name: "Shop" })[0] as HTMLElement);
    expect(opened).toContain("Shop");
  });
});

describe("keyboard", () => {
  it("reaches the column picker, the checkboxes, and every sortable header in order", async () => {
    const user = userEvent.setup();
    render(<Table rows={rows.slice(0, 5)} />);
    await user.tab();
    expect(screen.getByRole("button", { name: "Columns" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("checkbox", { name: "Select all 5" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Name" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(screen.getByRole("columnheader", { name: "Name" })).toHaveAttribute(
      "aria-sort",
      "ascending",
    );
    await user.tab();
    expect(screen.getByRole("button", { name: "Status" })).toHaveFocus();
  });
});

describe("states", () => {
  it("shows loading, error, and empty", async () => {
    const retry = vi.fn();
    const user = userEvent.setup();
    const { rerender } = render(
      <DataTable
        label="T"
        rows={[] as Row[]}
        columns={columns}
        getRowId={(r) => r.id}
        state="loading"
      />,
    );
    expect(screen.getByText("Loading…")).toBeInTheDocument();
    rerender(
      <DataTable
        label="T"
        rows={[] as Row[]}
        columns={columns}
        getRowId={(r) => r.id}
        state="error"
        error="No answer"
        onRetry={retry}
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("No answer");
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(retry).toHaveBeenCalled();
    rerender(<DataTable label="T" rows={[] as Row[]} columns={columns} getRowId={(r) => r.id} />);
    expect(screen.getByText("No records")).toBeInTheDocument();
    expect(screen.getByText("0–0 of 0 records")).toBeInTheDocument();
  });
});

describe("sortRows", () => {
  it("is stable and puts empty values last", () => {
    const data = [
      { id: "a", v: 2 },
      { id: "b", v: null },
      { id: "c", v: 1 },
      { id: "d", v: 2 },
    ];
    const cols: Column<(typeof data)[number]>[] = [
      { id: "v", header: "V", cell: (r) => r.v, sortValue: (r) => r.v },
    ];
    expect(sortRows(data, cols, { column: "v", direction: "asc" }).map((r) => r.id)).toEqual([
      "c",
      "a",
      "d",
      "b",
    ]);
    expect(sortRows(data, cols, { column: "v", direction: "desc" }).map((r) => r.id)).toEqual([
      "a",
      "d",
      "c",
      "b",
    ]);
  });
});
