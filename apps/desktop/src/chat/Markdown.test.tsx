import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { Markdown } from "./Markdown";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("Markdown", () => {
  it("draws formatting as elements", () => {
    const { container } = render(
      <Markdown
        text={
          "## The plan\n\nI saved **clear-temp.ps1** and *tested* `Get-ChildItem`.\n\n- one\n- two\n\n1. first\n2. second"
        }
      />,
    );
    expect(screen.getByRole("heading", { name: "The plan" })).toHaveAttribute("aria-level", "5");
    expect(container.querySelector("strong")).toHaveTextContent("clear-temp.ps1");
    expect(container.querySelector("em")).toHaveTextContent("tested");
    expect(container.querySelector("code.md__code")).toHaveTextContent("Get-ChildItem");
    expect(container.querySelectorAll("ul li")).toHaveLength(2);
    expect(container.querySelectorAll("ol li")).toHaveLength(2);
  });

  it("never turns anything an agent writes into live HTML", () => {
    const { container } = render(
      <Markdown text={"<script>alert(1)</script> <img src=x onerror=alert(1)> **ok**"} />,
    );
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect(container).toHaveTextContent("<script>alert(1)</script>");
  });

  it("links only web addresses and mail, and never takes over the window", async () => {
    const open = vi.fn();
    const { container } = render(
      <Markdown
        text={
          "[docs](https://example.com/a) and [bad](javascript:alert(1)) and [file](file:///C:/x)"
        }
        onOpenLink={open}
      />,
    );
    const links = container.querySelectorAll("a");
    expect(links).toHaveLength(1);
    expect(links[0]).toHaveAttribute("href", "https://example.com/a");
    expect(container).toHaveTextContent("bad");
    expect(container).toHaveTextContent("file");
    await userEvent.setup().click(screen.getByRole("link", { name: "docs" }));
    expect(open).toHaveBeenCalledWith("https://example.com/a");
  });

  it("shows a code block with its language, and copies it", async () => {
    // The test library puts its own clipboard in place when it is set up: ours comes after.
    const user = userEvent.setup();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    render(<Markdown text={"```powershell\nGet-ChildItem $env:TEMP\n```"} />);
    const figure = screen.getByText("powershell").closest("figure") as HTMLElement;
    expect(within(figure).getByText(/Get-ChildItem/)).toBeInTheDocument();
    await user.click(within(figure).getByRole("button", { name: "Copy" }));
    expect(writeText).toHaveBeenCalledWith("Get-ChildItem $env:TEMP");
    expect(await within(figure).findByRole("button", { name: "Copied" })).toBeInTheDocument();
  });

  it("says when the system would not copy", async () => {
    const user = userEvent.setup();
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText: vi.fn().mockRejectedValue(new Error("no")) },
      configurable: true,
    });
    render(<Markdown text={"```\nx\n```"} />);
    await user.click(screen.getByRole("button", { name: "Copy" }));
    expect(await screen.findByRole("button", { name: "Could not copy" })).toBeInTheDocument();
  });

  it("keeps a code block that is still being written, and says so", () => {
    render(<Markdown text={"```ts\nconst a ="} />);
    expect(screen.getByText("being written…")).toBeInTheDocument();
    expect(screen.getByText(/const a =/)).toBeInTheDocument();
  });

  it("draws a table with its alignment", () => {
    const { container } = render(<Markdown text={"| Name | Size |\n| :-- | --: |\n| a | 1 |"} />);
    const cells = container.querySelectorAll("td");
    expect(cells[0]).toHaveStyle({ textAlign: "left" });
    expect(cells[1]).toHaveStyle({ textAlign: "right" });
    expect(screen.getByRole("region", { name: "Table" })).toBeInTheDocument();
  });

  it("draws a task list with a mark and a word, not a color", () => {
    render(<Markdown text={"- [x] done\n- [ ] to do"} />);
    expect(screen.getByRole("img", { name: "done" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "not done" })).toBeInTheDocument();
  });

  it("does not draw the blocks that did not change again while words arrive", () => {
    const { rerender, container } = render(<Markdown text={"First paragraph.\n\nSecond"} />);
    const first = container.querySelectorAll("p")[0];
    rerender(<Markdown text={"First paragraph.\n\nSecond paragraph grows"} />);
    expect(container.querySelectorAll("p")[0]).toBe(first);
    expect(container).toHaveTextContent("Second paragraph grows");
  });
});
