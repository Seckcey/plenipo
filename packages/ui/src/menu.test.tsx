import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import { Tabs } from "./controls";
import { MenuButton, ResizeHandle } from "./menu";

describe("MenuButton", () => {
  const items = [
    { id: "pc", label: "This PC", icon: "terminal" as const },
    { id: "shop", label: "Shop", hint: "PRODUCTION" },
    { id: "old", label: "Old box", disabled: true },
    { id: "dev", label: "Dev box" },
  ];

  it("opens, moves with the arrow keys past what cannot be picked, and picks", async () => {
    const user = userEvent.setup();
    const picked: string[] = [];
    render(<MenuButton label="New terminal" items={items} onSelect={(id) => picked.push(id)} />);
    const button = screen.getByRole("button", { name: "New terminal" });
    expect(button).toHaveAttribute("aria-expanded", "false");
    await user.click(button);
    expect(button).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("menuitem", { name: "This PC" })).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("menuitem", { name: /Shop/ })).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("menuitem", { name: "Dev box" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(picked).toEqual(["dev"]);
    expect(screen.queryByRole("menu")).toBeNull();
    expect(button).toHaveFocus();
  });

  it("closes with Escape and with a click outside, and says when there is nothing", async () => {
    const user = userEvent.setup();
    render(
      <div>
        <MenuButton
          label="New terminal"
          items={[]}
          onSelect={() => undefined}
          empty="No servers yet"
        />
        <p>outside</p>
      </div>,
    );
    await user.click(screen.getByRole("button", { name: "New terminal" }));
    expect(screen.getByRole("menu")).toHaveTextContent("No servers yet");
    await user.click(screen.getByText("outside"));
    expect(screen.queryByRole("menu")).toBeNull();
    const button = screen.getByRole("button", { name: "New terminal" });
    button.focus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("menu")).toBeInTheDocument();
    // Nothing in it can take the focus: Escape still closes it, and so does Tab away.
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(button).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("menu")).toBeInTheDocument();
    await user.tab();
    expect(screen.queryByRole("menu")).toBeNull();
  });
});

describe("ResizeHandle", () => {
  function Panel({ edge }: { edge: "top" | "left" }) {
    const [size, setSize] = useState(240);
    return (
      <>
        <ResizeHandle
          label="Resize the terminal"
          value={size}
          min={120}
          max={400}
          edge={edge}
          onChange={setSize}
        />
        <output>{size}</output>
      </>
    );
  }

  it("grows and shrinks with the arrow keys, within its limits", async () => {
    const user = userEvent.setup();
    render(<Panel edge="top" />);
    const handle = screen.getByRole("separator", { name: "Resize the terminal" });
    expect(handle).toHaveAttribute("aria-orientation", "horizontal");
    handle.focus();
    await user.keyboard("{ArrowUp}");
    expect(screen.getByRole("status")).toHaveTextContent("256");
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(screen.getByRole("status")).toHaveTextContent("224");
    await user.keyboard("{End}");
    expect(handle).toHaveAttribute("aria-valuenow", "400");
    await user.keyboard("{Home}");
    expect(handle).toHaveAttribute("aria-valuenow", "120");
  });

  it("follows a drag: up (or left) makes the panel bigger", () => {
    render(<Panel edge="left" />);
    const handle = screen.getByRole("separator");
    handle.setPointerCapture = () => undefined;
    handle.releasePointerCapture = () => undefined;
    fireEvent.pointerDown(handle, { clientX: 500, pointerId: 1 });
    fireEvent.pointerMove(handle, { clientX: 440, pointerId: 1 });
    expect(handle).toHaveAttribute("aria-valuenow", "300");
    fireEvent.pointerMove(handle, { clientX: 900, pointerId: 1 });
    expect(handle).toHaveAttribute("aria-valuenow", "120");
    fireEvent.pointerUp(handle, { clientX: 900, pointerId: 1 });
  });
});

describe("Tabs with close buttons", () => {
  function Terminals() {
    const [tabs, setTabs] = useState(["pc", "shop", "dev"]);
    const [value, setValue] = useState("shop");
    const close = (t: string) => {
      setTabs((all) => all.filter((x) => x !== t));
      if (t === value) setValue(tabs.find((x) => x !== t) ?? "");
    };
    return (
      <Tabs
        label="Terminals"
        value={value}
        onChange={setValue}
        tabs={tabs.map((t) => ({
          value: t,
          label: t,
          onClose: () => close(t),
          closeLabel: `Close ${t}`,
        }))}
      />
    );
  }

  it("closes a tab with its button or with Delete, and keeps the focus in the tabs", async () => {
    const user = userEvent.setup();
    render(<Terminals />);
    expect(screen.getByRole("tab", { name: "shop" })).toHaveAttribute(
      "aria-keyshortcuts",
      "Delete",
    );
    await user.click(screen.getByRole("button", { name: "Close dev" }));
    expect(screen.queryByRole("tab", { name: "dev" })).toBeNull();
    // The button's tab went: the focus is on its neighbour, not lost.
    expect(screen.getByRole("tab", { name: "shop" })).toHaveFocus();
    await user.keyboard("{Delete}");
    expect(screen.queryByRole("tab", { name: "shop" })).toBeNull();
    expect(screen.getByRole("tab", { name: "pc" })).toHaveFocus();
  });

  it("lists sections down the side and moves with the up and down arrows", async () => {
    const user = userEvent.setup();
    const seen: string[] = [];
    render(
      <Tabs
        label="Settings"
        orientation="vertical"
        value="a"
        onChange={(v) => seen.push(v)}
        tabs={[
          { value: "a", label: "AI tools" },
          { value: "b", label: "Servers" },
        ]}
      />,
    );
    expect(screen.getByRole("tablist")).toHaveAttribute("aria-orientation", "vertical");
    screen.getByRole("tab", { name: "AI tools" }).focus();
    await user.keyboard("{ArrowDown}");
    await user.keyboard("{ArrowRight}");
    expect(seen).toEqual(["b"]);
  });
});
