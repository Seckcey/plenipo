import { render, screen, within } from "@testing-library/react";

import { bucketActivity, DAY_MS } from "./activity";
import {
  ActivityStrip,
  ActivityStripPlaceholder,
  CountBadge,
  HealthBar,
  Sparkline,
  StatusDot,
  StatusPill,
} from "./status";
import { STATUSES } from "./status-types";

describe("status without color", () => {
  it.each(STATUSES)("%s: a mark and its word are both in the page", (status) => {
    const { container } = render(
      <>
        <StatusDot status={status} label={`Word for ${status}`} />
        <StatusPill status={status} label={`Pill for ${status}`} />
      </>,
    );
    expect(screen.getByText(`Word for ${status}`)).toBeVisible();
    expect(screen.getByText(`Pill for ${status}`)).toBeVisible();
    const marks = container.querySelectorAll(".ui-status__mark");
    expect(marks).toHaveLength(2);
    for (const mark of marks) expect(mark).toHaveAttribute("aria-hidden", "true");
    expect(container.querySelector(`[data-status="${status}"]`)).not.toBeNull();
  });

  it("gives every status its own class, so each mark has its own shape", () => {
    const { container } = render(
      <>
        {STATUSES.map((s) => (
          <StatusDot key={s} status={s} label={s} />
        ))}
      </>,
    );
    const classes = [...container.querySelectorAll(".ui-status")].map((el) => el.className);
    expect(new Set(classes).size).toBe(STATUSES.length);
  });
});

describe("CountBadge", () => {
  it("speaks its meaning and hides at zero", () => {
    const { rerender } = render(<CountBadge count={3} label="waiting for you" />);
    expect(screen.getByLabelText("3 waiting for you")).toHaveTextContent("3");
    rerender(<CountBadge count={0} label="waiting for you" />);
    expect(screen.queryByLabelText("0 waiting for you")).toBeNull();
    rerender(<CountBadge count={0} label="waiting for you" showZero />);
    expect(screen.getByLabelText("0 waiting for you")).toBeInTheDocument();
  });
});

describe("HealthBar", () => {
  it("is a meter with its value in words", () => {
    render(<HealthBar label="Tasks done" value={7} max={10} valueText="7 of 10" />);
    const meter = screen.getByRole("meter", { name: "Tasks done" });
    expect(meter).toHaveAttribute("aria-valuenow", "7");
    expect(meter).toHaveAttribute("aria-valuetext", "7 of 10");
    expect(screen.getByText("7 of 10")).toBeVisible();
  });
});

describe("Sparkline", () => {
  it("speaks its range", () => {
    render(<Sparkline values={[1, 5, 3]} label="Events per hour" />);
    expect(
      screen.getByRole("img", { name: /Events per hour: lowest 1, highest 5, latest 3/ }),
    ).toBeInTheDocument();
  });

  it("says when there is no data", () => {
    render(<Sparkline values={[]} label="Events" />);
    expect(screen.getByRole("img", { name: "Events: no data yet" })).toHaveTextContent(
      "No data yet",
    );
  });
});

describe("ActivityStrip", () => {
  const now = Date.UTC(2026, 8, 27, 18, 0);
  const series = bucketActivity(
    [
      { at: now - 60_000, kind: "problem" },
      { at: now - DAY_MS + 1 },
      { at: now - 7_200_000, kind: "waiting" },
    ],
    now - DAY_MS,
    now,
  );

  it("draws one segment per bucket, with a spoken summary and a Now marker", () => {
    const { container } = render(
      <ActivityStrip series={series} now={now} label="Website activity" />,
    );
    const bar = screen.getByRole("img", {
      name: "Website activity. Last 24 hours: 3 events, 1 problem, 1 waiting for approval",
    });
    expect(bar.querySelectorAll(".ui-strip__seg")).toHaveLength(96);
    expect(container.querySelector(".ui-strip__seg--problem")).not.toBeNull();
    expect(container.querySelector(".ui-strip__seg--waiting")).not.toBeNull();
    expect(container.querySelector(".ui-strip__now")).not.toBeNull();
    expect(
      within(container.querySelector(".ui-strip__axis") as HTMLElement).getByText("Now"),
    ).toBeInTheDocument();
  });

  it("has empty, loading, and error placeholders", () => {
    render(
      <>
        <ActivityStripPlaceholder state="empty" />
        <ActivityStripPlaceholder state="loading" />
        <ActivityStripPlaceholder state="error" />
      </>,
    );
    expect(screen.getByText("No activity yet")).toBeInTheDocument();
    expect(screen.getByText("Loading activity…")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("Couldn't load activity");
  });
});
