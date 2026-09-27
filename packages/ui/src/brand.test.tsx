import { render, screen } from "@testing-library/react";

import { Pip, PlenipoLogo, PlenipoMark } from "./brand";
import { PIP_IMAGES, PIP_POSES } from "./brand-data";
import { EmptyState, ErrorState } from "./states";

describe("Plenipo's brand", () => {
  it("has all 15 of Pip's poses, each with its image and plain words", () => {
    const poses = PIP_POSES.map((p) => p.pose);
    expect(poses).toHaveLength(15);
    expect(new Set(poses).size).toBe(15);
    for (const p of PIP_POSES) {
      expect(PIP_IMAGES[p.pose], p.pose).toBeTruthy();
      expect(p.doing.length, p.pose).toBeGreaterThan(0);
    }
  });

  it("draws the P as decoration unless it is labeled", () => {
    const { container, rerender } = render(<PlenipoMark />);
    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
    expect(container.querySelectorAll(".ui-brand__rail")).toHaveLength(3);
    rerender(<PlenipoMark label="Plenipo" />);
    expect(screen.getByRole("img", { name: "Plenipo" })).toBeInTheDocument();
  });

  it("draws the logo: the P, lenipo, and Pip on the n", () => {
    const { container } = render(<PlenipoLogo height={40} />);
    const logo = screen.getByRole("img", { name: "Plenipo" });
    expect(logo).toHaveAttribute("height", "40");
    // 749 x 298 keeps the kit's proportions.
    expect(logo).toHaveAttribute("width", String(Math.round((40 * 749) / 298)));
    expect(container.querySelectorAll(".ui-brand__ink")).toHaveLength(5);
    expect(container.querySelectorAll(".ui-brand__o")).toHaveLength(1);
    expect(container.querySelector("image")).toHaveAttribute("href", PIP_IMAGES.coding);
  });

  it("draws the square logo without the name", () => {
    const { container } = render(<PlenipoLogo variant="square" height={120} label="Pip" />);
    expect(screen.getByRole("img", { name: "Pip" })).toHaveAttribute("width", "120");
    expect(container.querySelectorAll(".ui-brand__ink")).toHaveLength(0);
    expect(container.querySelectorAll(".ui-brand__rail")).toHaveLength(3);
  });

  it("shows Pip as decoration unless he is labeled", () => {
    const { container, rerender } = render(<Pip pose="welcome" />);
    const img = container.querySelector("img");
    expect(img).toHaveAttribute("alt", "");
    expect(img).toHaveAttribute("src", PIP_IMAGES.welcome);
    expect(img).toHaveAttribute("data-pip", "welcome");
    expect(img).toHaveClass("ui-pip--md");
    rerender(<Pip pose="support" size="lg" label="Pip, here to help" />);
    expect(screen.getByRole("img", { name: "Pip, here to help" })).toHaveClass("ui-pip--lg");
  });

  it("puts Pip beside an empty or failed state, but not in the compact size", () => {
    const { container, rerender } = render(
      <EmptyState pip="recharging" title="All quiet">
        Nothing is waiting for you.
      </EmptyState>,
    );
    expect(container.querySelector("[data-pip=recharging]")).not.toBeNull();
    expect(screen.getByRole("status")).toHaveTextContent("All quiet");
    rerender(<EmptyState pip="recharging" compact title="All quiet" />);
    expect(container.querySelector("[data-pip]")).toBeNull();
    rerender(<ErrorState pip="support" title="Couldn't load" />);
    expect(container.querySelector("[data-pip=support]")).not.toBeNull();
  });
});
