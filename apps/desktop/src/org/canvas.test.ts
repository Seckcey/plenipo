import { describe, expect, it } from "vitest";
import type { LiveView } from "@plenipo/types";

import { position, sampleOrganization } from "../test/orgFixtures";
import { MAX_COORDINATE, mergePlaces, movedPlaces } from "./arrange";
import { NO_FILTERS, filterOptions, visibleTiles } from "./filters";
import { ORG_ID, OWNER_ID, layoutOrganization, workerNodeId } from "./layout";
import { handoffMarks, whereLines } from "./live";
import { SYMBOLS, SYMBOL_GROUPS } from "./symbols";

describe("saved places (ADR-053 §3)", () => {
  it("puts a placed tile where it was put, and its team keeps its offset from it", () => {
    const snapshot = sampleOrganization();
    const auto = layoutOrganization(snapshot);
    const web = auto.byId.get("p-web")!;
    const dev = auto.byId.get("p-dev")!;
    snapshot.places = [{ tileId: "p-web", x: web.x + 500, y: web.y - 300 }];
    const placed = layoutOrganization(snapshot);
    expect(placed.byId.get("p-web")).toMatchObject({
      x: web.x + 500,
      y: web.y - 300,
      placed: true,
    });
    // A report never moved follows its lead, and so does its live worker.
    expect(placed.byId.get("p-dev")).toMatchObject({
      x: dev.x + 500,
      y: dev.y - 300,
      placed: false,
    });
    const worker = auto.byId.get(workerNodeId("a-dev-1"))!;
    expect(placed.byId.get(workerNodeId("a-dev-1"))).toMatchObject({
      x: worker.x + 500,
      y: worker.y - 300,
    });
    // Tiles elsewhere keep their automatic spot.
    expect(placed.byId.get("p-sec")).toMatchObject({ x: auto.byId.get("p-sec")!.x });
    // The team's lines still look as before; the lead's own line now curves to it.
    expect(placed.links.find((l) => l.id === "bus:p-web")).toBeDefined();
    expect(placed.links.find((l) => l.id === "link:p-dev")?.d).not.toContain(" C ");
    expect(placed.links.find((l) => l.id === "link:p-web")?.d).toContain(" C ");
    expect(placed.links.find((l) => l.id === "link:p-web")?.childId).toBe("p-web");
  });

  it("moves a tile with its team, or alone with Alt", () => {
    const snapshot = sampleOrganization();
    const auto = layoutOrganization(snapshot);
    const eng = auto.byId.get("p-eng")!;
    // With its team: only the lead gets a spot (its team follows by itself).
    expect(movedPlaces(auto, "p-eng", 40, 60, false)).toEqual([
      { tileId: "p-eng", x: eng.x + 40, y: eng.y + 60 },
    ]);
    // Alone: its direct reports are pinned where they are.
    const alone = movedPlaces(auto, "p-eng", 40, 60, true);
    expect(alone.map((p) => p.tileId).sort()).toEqual(["p-eng", "p-sec", "p-web"]);
    const after = layoutOrganization({ ...snapshot, places: alone });
    expect(after.byId.get("p-web")!.x).toBe(auto.byId.get("p-web")!.x);
    expect(after.byId.get("p-dev")!.x).toBe(auto.byId.get("p-dev")!.x);
    // A team member placed by hand moves along with its lead.
    const placedMember = layoutOrganization({
      ...snapshot,
      places: [{ tileId: "p-dev", x: 5000, y: 100 }],
    });
    expect(movedPlaces(placedMember, "p-eng", 10, 0, false)).toContainEqual({
      tileId: "p-dev",
      x: 5010,
      y: 100,
    });
    // Workers are never placed; spots stay within the Ledger's limits.
    expect(movedPlaces(auto, workerNodeId("a-dev-1"), 5, 5, false)).toEqual([]);
    expect(movedPlaces(auto, OWNER_ID, 1e9, -1e9, false)[0]).toMatchObject({
      x: MAX_COORDINATE,
      y: -MAX_COORDINATE,
    });
    expect(mergePlaces([{ tileId: "a", x: 1, y: 1 }], [{ tileId: "a", x: 2, y: 3 }])).toEqual([
      { tileId: "a", x: 2, y: 3 },
    ]);
  });

  it("draws a lent line from a lent agent to the lead it helps", () => {
    const snapshot = sampleOrganization();
    snapshot.positions = snapshot.positions.map((p) =>
      p.id === "p-sec"
        ? {
            ...p,
            loan: {
              toLeadId: "p-camp",
              to: "Campaign Supervisor",
              project: "Q4 Campaign",
              until: "objective",
              objectiveTaskId: null,
              goingHome: false,
              since: 0,
            },
          }
        : p,
    );
    const layout = layoutOrganization(snapshot);
    expect(layout.lent).toEqual([
      expect.objectContaining({ id: "lent:p-sec", positionId: "p-sec", toLeadId: "p-camp" }),
    ]);
  });
});

describe("filters (ADR-053 §13)", () => {
  it("each filter narrows the canvas, keeping the leads above a match, faded", () => {
    const snapshot = sampleOrganization();
    expect(visibleTiles(snapshot, NO_FILTERS)).toBeNull();
    const cases = [
      [{ department: "d-mkt" }, ["p-mkt", "p-camp", "p-design", "p-docs"]],
      [{ project: "pr-web" }, ["p-web", "p-dev", "p-review", "p-qa"]],
      [{ status: "unavailable" as const }, ["p-design"]],
      [{ runtime: "codex" }, ["p-design"]],
      [{ company: "OpenAI" }, ["p-design"]],
      [{ rank: "departmentManager" as const }, ["p-eng", "p-mkt"]],
    ] as const;
    for (const [filter, matches] of cases) {
      const r = visibleTiles(snapshot, { ...NO_FILTERS, ...filter })!;
      expect(r.matched, JSON.stringify(filter)).toBe(matches.length);
      for (const id of matches) expect(r.shown.has(id)).toBe(true);
      for (const id of matches) expect(r.faded.has(id)).toBe(false);
      // Leads above a match stay, faded; the rest is hidden.
      expect(r.shown.has("p-super")).toBe(true);
      expect(r.faded.has("p-super")).toBe(matches.includes("p-super" as never) ? false : true);
      const layout = layoutOrganization(snapshot, new Set(), { shown: r.shown });
      const shownPositions = layout.nodes.filter((n) => n.kind === "position").map((n) => n.id);
      expect(new Set(shownPositions)).toEqual(r.shown);
    }
    // A specialty narrows to the agents that have it.
    snapshot.positions = snapshot.positions.map((p) =>
      p.id === "p-dev" ? { ...p, specialtyId: "s-pay", specialty: "Payments" } : p,
    );
    expect(visibleTiles(snapshot, { ...NO_FILTERS, specialty: "s-pay" })?.matched).toBe(1);
    // The search narrows like a filter.
    expect(visibleTiles(snapshot, NO_FILTERS, ["p-docs"])?.shown).toEqual(
      new Set(["p-docs", "p-camp", "p-mkt", "p-super"]),
    );
    const options = filterOptions(snapshot);
    expect(options.company.map((o) => o.label)).toEqual(["Anthropic", "OpenAI"]);
    expect(options.specialty).toEqual([{ value: "s-pay", label: "Payments" }]);
    expect(options.rank.map((o) => o.value)).toContain("worker");
  });
});

describe("the live view (ADR-053 §17–§20)", () => {
  const live = (patch: Partial<LiveView> = {}): LiveView => ({
    workers: [],
    handoffs: [],
    at: 1_000_000,
    ...patch,
  });

  it("says where each worker thinks, runs, and what it touches, in each case", () => {
    const snapshot = sampleOrganization();
    const lines = whereLines(
      snapshot,
      live({
        workers: [
          {
            grantId: "g1",
            taskId: "task-a-dev-1",
            positionId: "p-dev",
            worker: "Senior Developer",
            runtimeId: "claude-code",
            touching: { kind: "folder", project: "Website Relaunch", folder: "src/pages" },
          },
          {
            grantId: "g2",
            taskId: "task-super",
            positionId: "p-super",
            worker: "VP",
            runtimeId: "codex",
            runsOn: { kind: "server", name: "Shop", production: true },
            touching: { kind: "server", name: "Shop", production: true },
          },
          {
            grantId: "g3",
            taskId: "t-other",
            positionId: "p-review",
            worker: "Code Reviewer",
            runtimeId: "claude-code",
            runsOn: { kind: "thisPc", what: "Plenipo's browser" },
            touching: { kind: "website", host: "shop.example.com" },
          },
        ],
      }),
    );
    // An on-call worker's line goes on its own worker tile.
    const dev = lines.get(workerNodeId("a-dev-1"));
    expect(dev?.thinksIn.words).toBe("Thinks in Anthropic's cloud");
    expect(dev?.runsOn).toBeNull();
    expect(dev?.touching).toEqual({
      symbol: "touch-folder",
      words: "Touching Website Relaunch · src/pages",
    });
    const vp = lines.get("p-super");
    expect(vp?.thinksIn.words).toBe("Thinks in OpenAI's cloud");
    expect(vp?.runsOn).toEqual({ symbol: "where-server", words: "Runs on Shop · PRODUCTION" });
    const reviewer = lines.get("p-review");
    expect(reviewer?.runsOn?.words).toBe("Runs on this PC (Plenipo's browser)");
    expect(reviewer?.touching?.words).toBe("Touching shop.example.com");
  });

  it("shows the last minute's hand-offs between shown tiles", () => {
    const layout = layoutOrganization(sampleOrganization());
    const marks = handoffMarks(
      layout,
      live({
        handoffs: [
          { id: "h1", kind: "asked", fromPositionId: "p-web", toPositionId: "p-dev", at: 990_000 },
          {
            id: "h2",
            kind: "answered",
            fromPositionId: "p-dev",
            toPositionId: "p-web",
            at: 999_000,
          },
          { id: "old", kind: "asked", fromPositionId: "p-web", toPositionId: "p-qa", at: 100 },
          { id: "gone", kind: "asked", fromPositionId: "p-web", toPositionId: "p-x", at: 999_000 },
        ],
      }),
    );
    expect(marks.map((m) => [m.id, m.kind])).toEqual([
      ["h1", "asked"],
      ["h2", "answered"],
    ]);
    expect(marks[0]?.d.startsWith("M ")).toBe(true);
  });
});

describe("the legend's list (ADR-053 §15)", () => {
  it("has one line per mark, each with words and a group", () => {
    const keys = SYMBOLS.map((s) => s.key);
    expect(new Set(keys).size).toBe(keys.length);
    for (const s of SYMBOLS) {
      expect(s.label.length).toBeGreaterThan(0);
      expect(s.words.length).toBeGreaterThan(10);
      expect(SYMBOL_GROUPS).toContain(s.group);
    }
    expect(ORG_ID).toBe("organization");
    expect(position("x", "X", "r-dev", null).loan).toBeNull();
  });

  it("describes the drop outlines as they are drawn: green, or red with the reason in words", () => {
    const words = (key: string) => SYMBOLS.find((s) => s.key === key)?.words ?? "";
    expect(words("drop-valid")).toBe("A green outline: dropping here opens its choices.");
    expect(words("drop-refused")).toBe(
      "A red outline, with the reason in words next to the pointer.",
    );
  });
});
