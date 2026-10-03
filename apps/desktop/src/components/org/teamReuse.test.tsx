import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { sampleOrganization } from "../../test/orgFixtures";
import { SetUpDevelopmentDialog } from "./OrgDialogs";
import { reusableWorkers } from "./teamReuse";

const JOBS = ["Senior Developer", "Code Reviewer", "QA Engineer", "Documentation Writer"];

describe("using the team you hired first (Phase 25, item 2.7)", () => {
  it("finds each job's worker already in the department, each used once", () => {
    const s = sampleOrganization();
    const found = reusableWorkers(s, "d-eng", JOBS).map((p) => p?.id ?? null);
    expect(found).toEqual(["p-dev", "p-review", "p-qa", null]);
    // Another department's writer isn't Engineering's.
    expect(reusableWorkers(s, "d-mkt", ["Documentation Writer"])[0]?.id).toBe("p-docs");
    expect(reusableWorkers(s, null, JOBS)).toEqual([null, null, null, null]);
  });

  it("the setup dialog offers each job's worker, or Hire new, and sends the choices", async () => {
    const onSubmit = vi.fn().mockResolvedValue(null);
    render(
      <SetUpDevelopmentDialog
        snapshot={sampleOrganization()}
        onCancel={() => undefined}
        onSubmit={onSubmit}
      />,
    );
    const user = userEvent.setup();
    const dialog = screen.getByRole("dialog", { name: "Set up a Development project" });
    await user.selectOptions(
      within(dialog).getByRole("combobox", { name: /^Department/ }),
      "d-eng",
    );
    const dev = within(dialog).getByRole("combobox", { name: "Senior Developer" });
    expect(
      within(dev)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual([expect.stringMatching(/^Use Senior Developer \(Senior Developer/), "Hire new"]);
    expect(dialog).toHaveTextContent("Documentation Writer: hire new (the department has none)");
    await user.selectOptions(
      within(dialog).getByRole("combobox", { name: "Code Reviewer" }),
      "Hire new",
    );
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Shop");
    await user.click(within(dialog).getByRole("button", { name: "Set up" }));
    expect(onSubmit).toHaveBeenCalledWith(
      expect.objectContaining({ departmentId: "d-eng", hireNew: ["Code Reviewer"] }),
    );
  });
});
