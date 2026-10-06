import { describe, expect, it } from "vitest";

import { sampleOrganization } from "../test/orgFixtures";
import { agentsIn, buildTree, busyCount, pathTo, type GroupRow, type TreeRow } from "./tree";

/** The tree as names, for reading at a glance: groups as [name, children], agents as titles. */
function names(rows: readonly TreeRow[]): unknown[] {
  return rows.map((r) => (r.kind === "agent" ? r.position.title : [r.name, names(r.children)]));
}

describe("the Workers page's tree (I4)", () => {
  it("shows departments, then their projects and teams, then each department's own staff", () => {
    expect(names(buildTree(sampleOrganization()))).toEqual([
      "VP",
      [
        "Engineering",
        [
          "Engineering Manager",
          [
            "Website Relaunch",
            // The Supervisor first, then the team in the organization's own order.
            ["Website Supervisor", "Code Reviewer", "QA Engineer", "Senior Developer"],
          ],
          "Security Auditor",
        ],
      ],
      [
        "Marketing",
        [
          "Marketing Manager",
          ["Q4 Campaign", ["Campaign Supervisor", "Designer", "Documentation Writer"]],
        ],
      ],
    ]);
  });

  it("shows each position once, and leaves out what is archived", () => {
    const org = sampleOrganization();
    org.positions = org.positions.map((p) => (p.id === "p-qa" ? { ...p, active: false } : p));
    org.projects = org.projects.map((p) => (p.id === "pr-camp" ? { ...p, active: false } : p));
    const tree = buildTree(org);
    const titles = agentsIn(tree).map((a) => a.position.title);
    expect(titles).not.toContain("QA Engineer");
    expect(new Set(titles).size).toBe(titles.length);
    // The archived project's people stay in the tree, under its department.
    const marketing = tree.find((r) => r.kind === "department" && r.name === "Marketing");
    expect(names(marketing ? [marketing] : [])).toEqual([
      [
        "Marketing",
        ["Marketing Manager", "Campaign Supervisor", "Designer", "Documentation Writer"],
      ],
    ]);
  });

  it("counts who is doing something now, and finds the way to a position", () => {
    const org = sampleOrganization();
    org.positions = org.positions.map((p) =>
      p.id === "p-dev" || p.id === "p-review"
        ? { ...p, status: p.id === "p-dev" ? "working" : "waiting" }
        : { ...p, status: "idle" },
    );
    const tree = buildTree(org);
    const engineering = tree.find((r): r is GroupRow => r.kind === "department");
    expect(engineering && busyCount(engineering)).toBe(2);
    expect(pathTo(tree, "p-dev")).toEqual(["department:d-eng", "project:pr-web"]);
    expect(pathTo(tree, "p-super")).toEqual([]);
    expect(pathTo(tree, "nobody")).toEqual([]);
  });
});
