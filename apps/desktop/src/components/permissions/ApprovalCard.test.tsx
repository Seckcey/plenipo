import { cleanup, render, screen } from "@testing-library/react";
import type { ApprovalView } from "@plenipo/types";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ApprovalCard } from "./ApprovalCard";

afterEach(cleanup);

const card = (overrides: Partial<ApprovalView> = {}): ApprovalView => ({
  id: "a1",
  taskId: "t1",
  status: "pending",
  requestedAt: 0,
  expiresAt: 600_000,
  resolvedAt: null,
  worker: "Operations Engineer",
  role: "Operations Engineer",
  capabilityLabel: "Connect to servers",
  summary: "run systemctl restart nginx on Shop",
  detail:
    "On: Shop — production server, shop@203.0.113.10:22\nWhere: in /var/www/shop\nRuns: systemctl restart nginx\nKind: Start, stop, and restart services",
  reason: "Shop is a production server: every command there waits for your approval.",
  riskLabel: "Runs commands on a server",
  waiting: true,
  server: "Shop",
  environment: "production",
  address: "shop@203.0.113.10:22",
  ...overrides,
});

describe("Approval card for a server (Phase 11)", () => {
  it("shows the server, production in red, and exactly what will run", () => {
    render(<ApprovalCard approval={card()} now={0} pending={false} onAnswer={vi.fn()} />);
    const article = screen.getByRole("article", {
      name: "Operations Engineer wants to run systemctl restart nginx on Shop",
    });
    expect(article).toHaveClass("approval--production");
    expect(screen.getByText("PRODUCTION")).toHaveClass("env--production");
    expect(article).toHaveTextContent("a production server: check exactly what will run");
    expect(screen.getByLabelText("Exactly what it will do")).toHaveTextContent(
      "Runs: systemctl restart nginx",
    );
  });

  it("shows a development server without the production warning", () => {
    render(
      <ApprovalCard
        approval={card({ environment: "development", server: "Dev box" })}
        now={0}
        pending={false}
        onAnswer={vi.fn()}
      />,
    );
    expect(screen.getByRole("article")).not.toHaveClass("approval--production");
    expect(screen.getByText("Development")).toHaveClass("env--development");
    expect(screen.queryByText(/check exactly what will run/)).toBeNull();
  });
});
