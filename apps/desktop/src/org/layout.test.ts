import { describe, expect, it } from "vitest";

import { emptyOrganization, sampleOrganization } from "../test/orgFixtures";
import {
  BUS_OFFSET,
  COLUMN_GAP,
  NODE_SIZE,
  ORG_ID,
  OWNER_ID,
  ancestorsOf,
  layoutOrganization,
  nodeAt,
  workerNodeId,
  type LayoutNode,
} from "./layout";

const center = (n: LayoutNode | undefined) => (n ? n.y + n.h / 2 : NaN);

describe("organization layout", () => {
  it("starts with the owner and the organization, like the uplink and the gateway", () => {
    const layout = layoutOrganization(emptyOrganization());
    expect(layout.nodes.map((n) => n.id)).toEqual([OWNER_ID, ORG_ID]);
    const [owner, org] = layout.nodes;
    expect(owner?.x).toBe(0);
    expect(org?.x).toBe(NODE_SIZE.owner.w + COLUMN_GAP);
    expect(center(owner)).toBe(center(org));
    expect(layout.toggles).toEqual([]);
  });

  it("places each team in the next column; a lead shares its first report's row", () => {
    const layout = layoutOrganization(sampleOrganization());
    const at = (id: string) => layout.byId.get(id);
    // Columns step to the right by depth.
    expect(at("p-super")?.depth).toBe(2);
    expect(at("p-eng")?.depth).toBe(3);
    expect(at("p-web")?.depth).toBe(4);
    expect(at("p-dev")?.depth).toBe(5);
    expect(at("p-dev")!.x).toBeGreaterThan(at("p-web")!.x);
    expect(at("p-web")?.x).toBe(at("p-sec")?.x);
    // The chain of command runs along one row.
    expect(center(at("p-super"))).toBe(center(at(ORG_ID)));
    expect(center(at("p-eng"))).toBe(center(at("p-super")));
    expect(center(at("p-web"))).toBe(center(at("p-eng")));
    // Later reports stack below, in order, without overlapping.
    const team = ["p-dev", "p-review", "p-qa"].map((id) => at(id)!);
    for (let i = 1; i < team.length; i++) {
      expect(team[i]!.y).toBeGreaterThanOrEqual(team[i - 1]!.y + team[i - 1]!.h);
    }
    const nodes = layout.nodes;
    for (const a of nodes) {
      for (const b of nodes) {
        if (a === b) continue;
        const overlap = a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
        expect(overlap, `${a.id} overlaps ${b.id}`).toBe(false);
      }
    }
  });

  it("hangs live workers off their position with dashed, runtime-labelled links", () => {
    const layout = layoutOrganization(sampleOrganization());
    const w = layout.byId.get(workerNodeId("a-dev-1"));
    expect(w?.kind).toBe("worker");
    expect(w?.parentId).toBe("p-dev");
    const link = layout.links.find((l) => l.id === `link:${workerNodeId("a-dev-1")}`);
    expect(link?.style).toBe("worker");
    expect(link?.active).toBe(true);
    const queued = layout.links.find((l) => l.id === `link:${workerNodeId("a-dev-2")}`);
    expect(queued?.active).toBe(false);
    expect(layout.chips.find((c) => c.id === `chip:${workerNodeId("a-qa-1")}`)?.label).toBe(
      "Codex",
    );
  });

  it("draws a bus from each lead and a branch to each report", () => {
    const layout = layoutOrganization(sampleOrganization());
    const web = layout.byId.get("p-web")!;
    const px = web.x + web.w;
    const bus = layout.links.find((l) => l.id === "bus:p-web");
    const qa = layout.byId.get("p-qa")!;
    expect(bus?.d).toBe(`M ${px} ${center(web)} H ${px + BUS_OFFSET} V ${center(qa) - 12}`);
    // The first report continues straight on; later ones curve off the bus.
    expect(layout.links.find((l) => l.id === "link:p-dev")?.d).toBe(
      `M ${px + BUS_OFFSET} ${center(web)} H ${layout.byId.get("p-dev")!.x}`,
    );
    expect(layout.links.find((l) => l.id === "link:p-qa")?.d).toContain(" Q ");
    // Working reports light up their link and the bus.
    expect(layout.links.find((l) => l.id === "link:p-qa")?.active).toBe(true);
    expect(bus?.active).toBe(true);
    expect(layout.links.find((l) => l.id === "link:p-review")?.active).toBe(false);
  });

  it("labels department heads and coordinators with their department and project", () => {
    const layout = layoutOrganization(sampleOrganization());
    const chip = (id: string) => layout.chips.find((c) => c.id === `chip:${id}`);
    expect(chip("p-eng")).toMatchObject({ label: "Engineering", tone: "department" });
    expect(chip("p-web")).toMatchObject({ label: "Website Relaunch", tone: "project" });
    expect(chip("p-dev")).toBeUndefined();
  });

  it("collapses a team behind its toggle and counts what is hidden", () => {
    const snapshot = sampleOrganization();
    const open = layoutOrganization(snapshot);
    expect(open.toggles.find((t) => t.id === "p-web")).toMatchObject({
      collapsed: false,
      count: 3,
    });
    const folded = layoutOrganization(snapshot, new Set(["p-web"]));
    for (const id of ["p-dev", "p-review", "p-qa", workerNodeId("a-dev-1")]) {
      expect(folded.byId.has(id)).toBe(false);
    }
    // Three reports and their three live workers.
    expect(folded.toggles.find((t) => t.id === "p-web")).toMatchObject({
      collapsed: true,
      hidden: 6,
    });
    expect(folded.links.some((l) => l.id === "stub:p-web")).toBe(true);
    // Oversight into a hidden team is not drawn.
    expect(folded.oversight.map((o) => o.id)).not.toContain("o-qa");
    expect(folded.bounds.h).toBeLessThan(open.bounds.h);
  });

  it("draws oversight between the overseer and the lead it oversees", () => {
    const layout = layoutOrganization(sampleOrganization());
    const sec = layout.oversight.find((o) => o.id === "o-sec");
    expect(sec).toMatchObject({ role: "security", overseerId: "p-sec", targetId: "p-web" });
    // Same column: a bracket in the gutter to the left.
    const x = layout.byId.get("p-sec")!.x;
    expect(sec?.d.startsWith(`M ${x} `)).toBe(true);
    expect(sec!.x).toBeLessThan(x);
  });

  it("finds nodes under a point and the path down to any node", () => {
    const snapshot = sampleOrganization();
    const layout = layoutOrganization(snapshot);
    const dev = layout.byId.get("p-dev")!;
    expect(nodeAt(layout, dev.x + 5, dev.y + 5)?.id).toBe("p-dev");
    expect(nodeAt(layout, dev.x - COLUMN_GAP / 2, dev.y + 5)).toBeNull();
    expect(ancestorsOf(snapshot, workerNodeId("a-dev-1"))).toEqual([
      OWNER_ID,
      ORG_ID,
      "p-super",
      "p-eng",
      "p-web",
      "p-dev",
    ]);
    expect(ancestorsOf(snapshot, ORG_ID)).toEqual([OWNER_ID]);
    expect(ancestorsOf(snapshot, OWNER_ID)).toEqual([]);
  });
});
