/**
 * The Phase 18 canvas (ADR-053, ADR-054, ADR-056): arranging with saved spots and Tidy up, the
 * trash can and the Archived drawer, rewiring by line ends, move or lend, filters, the legend,
 * the live view, the guide, and the owner's tile.
 */
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { LedgerEvent, LiveView, OrgSnapshot, OwnerProfile, WorkView } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import * as events from "../api/events";
import { layoutOrganization } from "../org/layout";
import { SYMBOLS, SYMBOL_KEYS } from "../org/symbols";
import { TOUR_KEY, TOUR_STEPS } from "../org/tour";
import { OwnerContext } from "../owner/context";
import { sampleOrganization } from "../test/orgFixtures";
import { sampleRouting } from "../test/routingFixtures";
import { OrganizationView } from "./OrganizationView";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getOrganization: vi.fn(),
    getWork: vi.fn(),
    getRouting: vi.fn(),
    getLiveView: vi.fn(),
    placeTiles: vi.fn(),
    tidyUp: vi.fn(),
    movePosition: vi.fn(),
    archivePosition: vi.fn(),
    archiveProject: vi.fn(),
    bringBack: vi.fn(),
    lendAgent: vi.fn(),
    sendHome: vi.fn(),
    retargetOversight: vi.fn(),
    cancelAgentTurn: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(),
  subscribeAgentUpdates: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeControl: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
const listeners = new Set<(e: LedgerEvent) => void>();

const noWork = (positionId: string | null): WorkView => ({
  positionId,
  running: [],
  waiting: [],
  queued: [],
  recent: [],
  team: [],
});

const quiet = (): LiveView => ({ workers: [], handoffs: [], at: Date.now() });

let reduceMotion = true;

function show(snapshot: OrgSnapshot = sampleOrganization(), owner: OwnerProfile | null = null) {
  api.getOrganization.mockResolvedValue(snapshot);
  const view = <OrganizationView onOpenSession={vi.fn()} onOpenTask={vi.fn()} />;
  return render(
    owner ? (
      <OwnerContext.Provider value={{ profile: owner, save: vi.fn() }}>
        {view}
      </OwnerContext.Provider>
    ) : (
      view
    ),
  );
}

/** Where a world point is on screen now (read from the canvas's own transform). */
function toScreen(wx: number, wy: number): { clientX: number; clientY: number } {
  const world = document.querySelector<HTMLElement>(".topology__world");
  const m = /translate\((-?[\d.]+)px, (-?[\d.]+)px\) scale\(([\d.]+)\)/.exec(
    world?.style.transform ?? "",
  );
  if (!m) throw new Error("no camera");
  const [tx, ty, z] = [Number(m[1]), Number(m[2]), Number(m[3])];
  return { clientX: tx + wx * z, clientY: ty + wy * z };
}

/** The world point at a screen point now (the inverse of `toScreen`). */
function toWorld(p: { clientX: number; clientY: number }): { x: number; y: number } {
  const world = document.querySelector<HTMLElement>(".topology__world");
  const m = /translate\((-?[\d.]+)px, (-?[\d.]+)px\) scale\(([\d.]+)\)/.exec(
    world?.style.transform ?? "",
  );
  if (!m) throw new Error("no camera");
  const [tx, ty, z] = [Number(m[1]), Number(m[2]), Number(m[3])];
  return { x: (p.clientX - tx) / z, y: (p.clientY - ty) / z };
}

/** A box on screen, as `getBoundingClientRect` gives it (nothing is measured in tests). */
function box(left: number, top: number, width: number, height: number): () => DOMRect {
  return () =>
    ({
      left,
      top,
      right: left + width,
      bottom: top + height,
      width,
      height,
      x: left,
      y: top,
    }) as DOMRect;
}

/** The words next to the pointer while dragging. */
function ghost(): Element | null {
  return document.querySelector(".topo-ghost");
}

function canvas(): HTMLElement {
  return screen.getByRole("region", { name: "Organization topology" });
}

/** Start dragging `el` from `start` (past the threshold), then point at `to`. */
function pointAt(
  el: Element,
  start: { clientX: number; clientY: number },
  to: { clientX: number; clientY: number },
) {
  fireEvent.pointerDown(el, { pointerId: 1, button: 0, ...start });
  fireEvent.pointerMove(window, {
    pointerId: 1,
    clientX: start.clientX + 20,
    clientY: start.clientY + 20,
  });
  fireEvent.pointerMove(window, { pointerId: 1, ...to });
}

function zoom(): number {
  const world = document.querySelector<HTMLElement>(".topology__world");
  return Number(/scale\(([\d.]+)\)/.exec(world?.style.transform ?? "")?.[1] ?? 1);
}

function centerOf(snapshot: OrgSnapshot, id: string) {
  const n = layoutOrganization(snapshot).byId.get(id);
  if (!n) throw new Error(`no node ${id}`);
  return toScreen(n.x + n.w / 2, n.y + n.h / 2);
}

function dragTo(
  from: Element,
  start: { clientX: number; clientY: number },
  to: { clientX: number; clientY: number },
  extra: { altKey?: boolean } = {},
) {
  fireEvent.pointerDown(from, { pointerId: 1, button: 0, ...start, ...extra });
  fireEvent.pointerMove(window, {
    pointerId: 1,
    clientX: start.clientX + 20,
    clientY: start.clientY + 20,
    ...extra,
  });
  fireEvent.pointerMove(window, { pointerId: 1, ...to, ...extra });
  fireEvent.pointerUp(window, { pointerId: 1, button: 0, ...to, ...extra });
}

/** The trash can's place on screen (nothing is measured in tests). */
function placeTrash() {
  const trash = screen.getByRole("button", { name: /^Trash can/ });
  trash.getBoundingClientRect = () =>
    ({
      left: 900,
      right: 930,
      top: 10,
      bottom: 40,
      width: 30,
      height: 30,
      x: 900,
      y: 10,
    }) as DOMRect;
  return { clientX: 915, clientY: 25 };
}

beforeEach(() => {
  sessionStorage.clear();
  localStorage.clear();
  localStorage.setItem(TOUR_KEY, "seen");
  reduceMotion = true;
  // Reduce motion: the camera moves at once, and hand-offs are still.
  window.matchMedia = vi.fn((query: string) => ({
    matches: reduceMotion && query.includes("reduce"),
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
  api.getWork.mockImplementation((id) => Promise.resolve(noWork(id ?? null)));
  api.getRouting.mockResolvedValue(sampleRouting());
  api.getLiveView.mockResolvedValue(quiet());
  api.placeTiles.mockResolvedValue(undefined);
  listeners.clear();
  vi.mocked(events.subscribeLedgerEvents).mockImplementation((handler) => {
    listeners.add(handler);
    return Promise.resolve(() => listeners.delete(handler));
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("arranging the canvas (ADR-053 §1–§6)", () => {
  it("saves where a tile is dropped, keeps it after a restart, and Tidy up (with Undo) puts it back", async () => {
    const org = sampleOrganization();
    show(org);
    const sec = await screen.findByRole("button", { name: "Security Auditor, Idle" });
    const layout = layoutOrganization(org);
    const node = layout.byId.get("p-sec")!;
    const start = centerOf(org, "p-sec");
    // Far below everything: an empty spot.
    const to = toScreen(node.x + node.w / 2, layout.bounds.y + layout.bounds.h + 400);
    dragTo(sec, start, to);
    await waitFor(() => expect(api.placeTiles).toHaveBeenCalledTimes(1));
    const z = zoom();
    const [saved] = api.placeTiles.mock.calls[0]![0];
    expect(saved?.tileId).toBe("p-sec");
    expect(saved?.x).toBeCloseTo(node.x + (to.clientX - start.clientX) / z, -1);
    expect(saved?.y).toBeCloseTo(node.y + (to.clientY - start.clientY) / z, -1);
    // The tile is drawn there at once.
    await waitFor(() => expect(sec.style.top).toBe(`${saved?.y}px`));
    cleanup();

    // After a restart the organization brings the spot back.
    const placed = { ...sampleOrganization(), places: [saved!], generatedAt: Date.now() - 1000 };
    api.tidyUp.mockResolvedValue([saved!]);
    show(placed);
    const again = await screen.findByRole("button", { name: "Security Auditor, Idle" });
    expect(again.style.top).toBe(`${saved?.y}px`);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /Tidy up/ }));
    expect(api.tidyUp).toHaveBeenCalled();
    await waitFor(() => expect(again.style.top).toBe(`${node.y}px`));
    const note = await screen.findByText(/Tidied up/);
    await user.click(within(note).getByRole("button", { name: "Undo" }));
    await waitFor(() => expect(api.placeTiles).toHaveBeenLastCalledWith([saved]));
    await waitFor(() => expect(again.style.top).toBe(`${saved?.y}px`));
  });

  it("moves the selected tile with Alt and the arrow keys, and arranges without menus", async () => {
    const org = sampleOrganization();
    show(org);
    const user = userEvent.setup();
    const eng = await screen.findByRole("button", { name: "Engineering Manager, Idle" });
    await user.click(eng);
    const node = layoutOrganization(org).byId.get("p-eng")!;
    fireEvent.keyDown(eng, { key: "ArrowDown", altKey: true });
    await waitFor(() =>
      expect(api.placeTiles).toHaveBeenCalledWith([{ tileId: "p-eng", x: node.x, y: node.y + 20 }]),
    );
    // Its team followed without spots of its own.
    const web = layoutOrganization(org).byId.get("p-web")!;
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /^Website Supervisor/ }).style.top).toBe(
        `${web.y + 20}px`,
      ),
    );

    // Arrange (A): dropping a tile on another only places it — no menu.
    fireEvent.keyDown(screen.getByRole("region", { name: "Organization topology" }), { key: "a" });
    expect(screen.getByRole("button", { name: "Arrange" })).toHaveAttribute("aria-pressed", "true");
    const sec = screen.getByRole("button", { name: "Security Auditor, Idle" });
    dragTo(sec, centerOf(org, "p-sec"), centerOf(org, "p-camp"), { altKey: true });
    await waitFor(() => expect(api.placeTiles).toHaveBeenCalledTimes(2));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });
});

describe("the trash can and the Archived drawer (ADR-053 §9–§11)", () => {
  it("archives an agent dropped on it, with Undo; a Supervisor asks about its project first", async () => {
    const org = sampleOrganization();
    show(org);
    api.archivePosition.mockResolvedValue(org);
    api.bringBack.mockResolvedValue(org);
    api.archiveProject.mockResolvedValue(org);
    const reviewer = await screen.findByRole("button", { name: "Code Reviewer, Idle" });
    const trash = placeTrash();
    dragTo(reviewer, centerOf(org, "p-review"), trash);
    await waitFor(() => expect(api.archivePosition).toHaveBeenCalledWith("p-review"));
    const user = userEvent.setup();
    const note = await screen.findByText(/Archived Code Reviewer\./);
    await user.click(within(note).getByRole("button", { name: "Undo" }));
    expect(api.bringBack).toHaveBeenCalledWith("position", "p-review");

    const supervisor = screen.getByRole("button", { name: /^Campaign Supervisor/ });
    dragTo(supervisor, centerOf(org, "p-camp"), placeTrash());
    const dialog = await screen.findByRole("dialog", {
      name: "Archive the Q4 Campaign project and its team?",
    });
    await user.click(within(dialog).getByRole("button", { name: "Archive" }));
    expect(api.archiveProject).toHaveBeenCalledWith("pr-camp");
    const done = await screen.findByText(/Archived Q4 Campaign\./);
    await user.click(within(done).getByRole("button", { name: "Undo" }));
    expect(api.bringBack).toHaveBeenCalledWith("project", "pr-camp");
  });

  it("refuses a lead with a team, with the reason, and opens the Archived drawer", async () => {
    const org = sampleOrganization();
    const archived = sampleOrganization();
    archived.positions = archived.positions.map((p) =>
      p.id === "p-docs" ? { ...p, active: false, status: "archived", archivedAt: 1 } : p,
    );
    show(archived);
    const vp = await screen.findByRole("button", { name: "VP, Working" });
    const trash = placeTrash();
    fireEvent.pointerDown(vp, { pointerId: 1, button: 0, ...centerOf(org, "p-super") });
    fireEvent.pointerMove(window, { pointerId: 1, clientX: 400, clientY: 400 });
    fireEvent.pointerMove(window, { pointerId: 1, ...trash });
    expect(screen.getByRole("status", { name: "" })).toHaveTextContent(
      "VP leads departments: archive each department first",
    );
    expect(screen.getByRole("button", { name: /^Trash can/ })).toHaveClass("is-drop-refused");
    fireEvent.pointerUp(window, { pointerId: 1, button: 0, ...trash });
    expect(api.archivePosition).not.toHaveBeenCalled();

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /^Trash can/ }));
    const drawer = screen.getByRole("region", { name: "Archived" });
    const row = within(drawer).getByRole("table", { name: "Archived agents" });
    expect(within(row).getByText("Documentation Writer")).toBeInTheDocument();
    api.bringBack.mockResolvedValue(org);
    await user.click(within(row).getByRole("button", { name: /Bring back/ }));
    expect(api.bringBack).toHaveBeenCalledWith("position", "p-docs");
  });
});

describe("rewiring by line ends (ADR-053 §7–§8)", () => {
  it("drags the selected agent's line end to a new lead, and refuses one the rules refuse", async () => {
    const org = sampleOrganization();
    show(org);
    api.movePosition.mockResolvedValue(org);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Senior Developer, Working" }));
    const handle = screen.getByRole("button", {
      name: /^Line end: Senior Developer reports to Website Supervisor/,
    });
    const link = layoutOrganization(org).links.find((l) => l.childId === "p-dev")!;
    const start = toScreen(link.from!.x, link.from!.y);
    // Onto an on-call agent: refused, with the reason.
    fireEvent.pointerDown(handle, { pointerId: 1, button: 0, ...start });
    fireEvent.pointerMove(window, {
      pointerId: 1,
      clientX: start.clientX + 30,
      clientY: start.clientY,
    });
    fireEvent.pointerMove(window, { pointerId: 1, ...centerOf(org, "p-review") });
    expect(screen.getByRole("status", { name: "" })).toHaveTextContent(
      "Code Reviewer is an on-call position",
    );
    fireEvent.pointerMove(window, { pointerId: 1, ...centerOf(org, "p-camp") });
    fireEvent.pointerUp(window, { pointerId: 1, button: 0, ...centerOf(org, "p-camp") });
    await waitFor(() => expect(api.movePosition).toHaveBeenCalledWith("p-dev", "p-camp"));
  });

  it("moves an oversight line's team end in one step, and offers the choices from the keyboard", async () => {
    const org = sampleOrganization();
    show(org);
    api.retargetOversight.mockResolvedValue(org);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Security Auditor, Idle" }));
    const teamEnd = screen.getByRole("button", {
      name: /^Line end: Website Supervisor's team, checked by Security Auditor/,
    });
    teamEnd.focus();
    await user.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: "Move this oversight line" });
    await user.click(
      within(menu).getByRole("menuitem", { name: "Check Campaign Supervisor's team" }),
    );
    expect(api.retargetOversight).toHaveBeenCalledWith("o-sec", { targetId: "p-camp" });
    expect(
      await screen.findByText(/Security Auditor is now Campaign Supervisor's security auditor/),
    ).toBeInTheDocument();
  });
});

describe("move or lend (ADR-054)", () => {
  it("lends an on-call agent for one objective, shows it lent, and sends it home", async () => {
    const org = sampleOrganization();
    const lent = sampleOrganization();
    lent.positions = lent.positions.map((p) =>
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
              since: 1,
            },
          }
        : p,
    );
    show(org);
    api.lendAgent.mockResolvedValue(lent);
    api.sendHome.mockResolvedValue(org);
    const auditor = await screen.findByRole("button", { name: "Security Auditor, Idle" });
    dragTo(auditor, centerOf(org, "p-sec"), centerOf(org, "p-camp"));
    const user = userEvent.setup();
    const menu = await screen.findByRole("menu", {
      name: "Security Auditor → Campaign Supervisor",
    });
    await user.click(within(menu).getByRole("menuitem", { name: /^Lend for one objective/ }));
    expect(api.lendAgent).toHaveBeenCalledWith("p-sec", "p-camp", "objective");
    // A lent badge, a dashed lent line with its label.
    const badge = await screen.findByRole("button", {
      name: /^Security Auditor is lent to Campaign Supervisor's team/,
    });
    expect(document.querySelector('[data-symbol="line-lent"]')).not.toBeNull();
    await user.click(badge);
    await user.click(await screen.findByRole("menuitem", { name: /^Send home/ }));
    expect(api.sendHome).toHaveBeenCalledWith("p-sec");
    expect(await screen.findByText("Security Auditor is home.")).toBeInTheDocument();
  });
});

describe("filters, the legend, the live view, and the guide (ADR-053 §13–§21)", () => {
  it("each filter narrows the canvas, keeping the leads above a match faded", async () => {
    show();
    const user = userEvent.setup();
    await screen.findByRole("button", { name: "VP, Working" });
    await user.click(screen.getByRole("button", { name: "Filters" }));
    const panel = screen.getByRole("region", { name: "Filters" });
    await user.selectOptions(within(panel).getByRole("combobox", { name: "AI company" }), "OpenAI");
    expect(within(panel).getByRole("status")).toHaveTextContent("Showing 1 of 11 agents");
    expect(screen.getByRole("button", { name: "Designer, Unavailable" })).not.toHaveClass(
      "is-dimmed",
    );
    expect(screen.getByRole("button", { name: "Campaign Supervisor, Idle" })).toHaveClass(
      "is-dimmed",
    );
    expect(
      screen.queryByRole("button", { name: "Engineering Manager, Idle" }),
    ).not.toBeInTheDocument();
    await user.click(within(panel).getByRole("button", { name: "Clear filters" }));
    expect(screen.getByRole("button", { name: "Engineering Manager, Idle" })).toBeInTheDocument();
  });

  it("the legend lists every mark a full canvas shows, and is remembered when hidden", async () => {
    const org = sampleOrganization();
    org.positions = org.positions.map((p) =>
      p.id === "p-sec"
        ? {
            ...p,
            specialtyId: "s1",
            specialty: "Payments",
            experience: { ...p.experience, experienced: true },
            loan: {
              toLeadId: "p-camp",
              to: "Campaign Supervisor",
              project: "Q4 Campaign",
              until: "returned",
              objectiveTaskId: null,
              goingHome: false,
              since: 1,
            },
          }
        : p.id === "p-web"
          ? { ...p, automatic: true }
          : p,
    );
    api.getLiveView.mockResolvedValue({
      at: Date.now(),
      workers: [
        {
          grantId: "g1",
          taskId: "task-super",
          positionId: "p-super",
          worker: "VP",
          runtimeId: "claude-code",
          runsOn: { kind: "server", name: "Shop", production: true },
          touching: { kind: "folder", project: "Website Relaunch", folder: "src" },
        },
        {
          grantId: "g2",
          taskId: "t2",
          positionId: "p-review",
          worker: "Code Reviewer",
          runtimeId: "claude-code",
          runsOn: { kind: "thisPc", what: "the screen" },
          touching: { kind: "website", host: "shop.example.com" },
        },
        {
          grantId: "g3",
          taskId: "t3",
          positionId: "p-docs",
          worker: "Documentation Writer",
          runtimeId: "claude-code",
          touching: { kind: "screen" },
        },
      ],
      handoffs: [
        { id: "h1", kind: "asked", fromPositionId: "p-web", toPositionId: "p-dev", at: Date.now() },
      ],
    });
    show(org);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Where" }));
    await user.click(screen.getByRole("button", { name: "Senior Developer, Working" }));
    await screen.findByText("Runs on Shop · PRODUCTION");
    const canvas = screen.getByRole("region", { name: "Organization topology" });
    const shown = new Set(
      [...canvas.querySelectorAll("[data-symbol]")]
        .filter((el) => !el.closest(".canvas-panel"))
        .map((el) => el.getAttribute("data-symbol")),
    );
    // Every mark on the canvas has its line in the legend.
    for (const key of shown)
      expect(SYMBOL_KEYS.has(key ?? ""), `${key} is in the legend`).toBe(true);
    for (const key of [
      "tile-owner",
      "tile-organization",
      "tile-full-time",
      "tile-on-call",
      "tile-worker",
      "line-reports",
      "line-worker",
      "line-active",
      "line-security",
      "line-lent",
      "line-handoff",
      "chip-department",
      "chip-project",
      "chip-runtime",
      "badge-lent",
      "badge-fixed-tool",
      "badge-specialty",
      "badge-experienced",
      "badge-oversees",
      "where-cloud",
      "where-server",
      "where-this-pc",
      "touch-folder",
      "touch-website",
      "touch-screen",
      "handle",
      "toggle",
      "status-working",
      "status-idle",
    ]) {
      expect(shown.has(key), `${key} shows on a full canvas`).toBe(true);
    }
    await user.click(screen.getByRole("button", { name: "Legend" }));
    expect(localStorage.getItem("plenipo.canvasLegend")).toBe("shown");
    const legend = screen.getByRole("region", { name: "Legend" });
    for (const s of SYMBOLS) {
      expect(legend.querySelector(`[data-legend="${s.key}"]`), s.key).not.toBeNull();
    }
    // Hidden, and remembered.
    await user.click(screen.getByRole("button", { name: "Legend" }));
    expect(screen.queryByRole("region", { name: "Legend" })).not.toBeInTheDocument();
    expect(localStorage.getItem("plenipo.canvasLegend")).toBe("hidden");
  });

  it("hand-offs move along the line, or stand still with reduce motion", async () => {
    const live: LiveView = {
      at: Date.now(),
      workers: [],
      handoffs: [
        { id: "h1", kind: "asked", fromPositionId: "p-web", toPositionId: "p-dev", at: Date.now() },
      ],
    };
    api.getLiveView.mockResolvedValue(live);
    reduceMotion = false;
    show();
    const moving = () =>
      document.getElementsByTagNameNS("http://www.w3.org/2000/svg", "animateMotion");
    const chip = () => document.querySelector(".topo-chip--handoff");
    await waitFor(() => expect(chip()).toHaveTextContent("Hand-off"));
    expect(moving()).toHaveLength(1);
    cleanup();
    reduceMotion = true;
    show();
    await waitFor(() => expect(chip()).toHaveTextContent("Hand-off"));
    expect(moving()).toHaveLength(0);
    const arrows = [...document.getElementsByClassName("topo-handoff__arrow")];
    expect(arrows.map((a) => a.getAttribute("transform"))).toEqual([
      expect.stringContaining("rotate("),
    ]);
  });

  it("reads the live view again when Guard or Liaison records something", async () => {
    show();
    await screen.findByRole("button", { name: "VP, Working" });
    await waitFor(() => expect(api.getLiveView).toHaveBeenCalledTimes(1));
    act(() =>
      listeners.forEach((l) =>
        l({
          seq: 1,
          id: "e1",
          taskId: "t",
          executionId: null,
          source: "guard",
          destination: null,
          eventType: "capability.used",
          payload: {},
          createdAt: 1,
        }),
      ),
    );
    await waitFor(() => expect(api.getLiveView).toHaveBeenCalledTimes(2), { timeout: 2000 });
  });

  it("shows a first-time tour of six steps, and ? starts it again", async () => {
    localStorage.removeItem(TOUR_KEY);
    show();
    const user = userEvent.setup();
    const tour = await screen.findByRole("dialog", { name: TOUR_STEPS[0]!.title });
    for (let i = 1; i < TOUR_STEPS.length; i++) {
      await user.click(within(tour).getByRole("button", { name: "Next" }));
      expect(within(tour).getByText(`Step ${i + 1} of ${TOUR_STEPS.length}`)).toBeInTheDocument();
    }
    await user.click(within(tour).getByRole("button", { name: "Done" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(localStorage.getItem(TOUR_KEY)).toBe("seen");
    await user.click(screen.getByRole("button", { name: "Guide to the canvas" }));
    await user.click(screen.getByRole("button", { name: "Take the tour again" }));
    expect(screen.getByRole("dialog", { name: TOUR_STEPS[0]!.title })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Skip the tour" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});

describe("the owner's tile (ADR-056)", () => {
  it("shows your picture, status, mood, and message on the canvas", async () => {
    show(sampleOrganization(), {
      status: "busy",
      mood: "great",
      message: "Feeling great!",
      picture: "iVBORw0KGgo=",
    });
    const you = await screen.findByRole("button", {
      name: "You, President: Busy, feeling Great, “Feeling great!”",
    });
    expect(within(you).getByRole("img", { name: "Your picture" })).toHaveAttribute(
      "src",
      "data:image/png;base64,iVBORw0KGgo=",
    );
    expect(you).toHaveTextContent("Busy");
    expect(you).toHaveTextContent("Great");
    expect(you).toHaveTextContent("Feeling great!");
  });
});

describe("where a drop lands (ADR-053 §2–§11)", () => {
  it("never drops on the trash can where it is under the details panel", async () => {
    const org = sampleOrganization();
    show(org);
    api.archivePosition.mockResolvedValue(org);
    const user = userEvent.setup();
    const reviewer = await screen.findByRole("button", { name: "Code Reviewer, Idle" });
    await user.click(reviewer);
    expect(screen.getByRole("complementary", { name: "Details: Code Reviewer" })).toBeVisible();
    // The toolbar and the panels keep clear of the details panel (360 wide by default).
    expect(canvas().style.getPropertyValue("--canvas-inset")).toBe("360px");
    // A 960-wide canvas: the trash can (at 900) is under the details panel.
    canvas().getBoundingClientRect = box(0, 0, 960, 640);
    const trash = placeTrash();
    pointAt(reviewer, centerOf(org, "p-review"), trash);
    expect(screen.getByRole("button", { name: /^Trash can/ })).not.toHaveClass("is-drop-target");
    fireEvent.pointerUp(window, { pointerId: 1, button: 0, ...trash });
    expect(api.archivePosition).not.toHaveBeenCalled();
  });

  it("does not scroll the view while an agent is held over the trash can", async () => {
    const org = sampleOrganization();
    show(org);
    const reviewer = await screen.findByRole("button", { name: "Code Reviewer, Idle" });
    canvas().getBoundingClientRect = box(0, 0, 960, 640);
    // The trash can is in the top edge, where holding a drag scrolls the view.
    const trash = placeTrash();
    const transform = () =>
      document.querySelector<HTMLElement>(".topology__world")!.style.transform;
    pointAt(reviewer, centerOf(org, "p-review"), trash);
    const before = transform();
    await new Promise((r) => setTimeout(r, 120));
    expect(transform()).toBe(before);
    // Near the top edge elsewhere, it does scroll.
    fireEvent.pointerMove(window, { pointerId: 1, clientX: 600, clientY: 20 });
    await waitFor(() => expect(transform()).not.toBe(before));
    fireEvent.keyDown(document.body, { key: "Escape" });
  });

  it("never drops on a tile hidden under a panel", async () => {
    const org = sampleOrganization();
    show(org);
    const user = userEvent.setup();
    const auditor = await screen.findByRole("button", { name: "Security Auditor, Idle" });
    await user.click(screen.getByRole("button", { name: "Filters" }));
    // The Filters panel covers Campaign Supervisor.
    const hidden = centerOf(org, "p-camp");
    screen.getByRole("region", { name: "Filters" }).getBoundingClientRect = box(
      hidden.clientX - 60,
      hidden.clientY - 60,
      120,
      120,
    );
    pointAt(auditor, centerOf(org, "p-sec"), hidden);
    expect(screen.getByRole("button", { name: /^Campaign Supervisor/ })).not.toHaveClass(
      "is-drop-target",
    );
    expect(ghost()).toHaveTextContent("Drop on a position · Esc cancels");
    fireEvent.pointerUp(window, { pointerId: 1, button: 0, ...hidden });
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("says what releasing does: archive, place, or where a line's end goes", async () => {
    const org = sampleOrganization();
    show(org);
    const layout = layoutOrganization(org);
    const escape = () => fireEvent.keyDown(document.body, { key: "Escape" });
    const reviewer = await screen.findByRole("button", { name: "Code Reviewer, Idle" });
    const trash = placeTrash();
    pointAt(reviewer, centerOf(org, "p-review"), trash);
    expect(ghost()).toHaveTextContent("Release to archive it (with Undo)");
    const empty = toScreen(layout.bounds.x + 40, layout.bounds.y + layout.bounds.h + 300);
    fireEvent.pointerMove(window, { pointerId: 1, ...empty });
    expect(ghost()).toHaveTextContent("Release to place it here · Alt: alone · Esc cancels");
    escape();
    expect(ghost()).toBeNull();

    // A Supervisor or a Manager asks first.
    const supervisor = screen.getByRole("button", { name: /^Campaign Supervisor/ });
    pointAt(supervisor, centerOf(org, "p-camp"), trash);
    expect(ghost()).toHaveTextContent("Release to archive its project (asks first)");
    escape();
    const manager = screen.getByRole("button", { name: /^Engineering Manager/ });
    pointAt(manager, centerOf(org, "p-eng"), trash);
    expect(ghost()).toHaveTextContent("Release to archive its department (asks first)");
    escape();

    // A line's end over an empty spot.
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Senior Developer, Working" }));
    const handle = screen.getByRole("button", { name: /^Line end: Senior Developer reports/ });
    const link = layoutOrganization(org).links.find((l) => l.childId === "p-dev")!;
    pointAt(handle, toScreen(link.from!.x, link.from!.y), empty);
    expect(ghost()).toHaveTextContent("Drop the line's end on an agent · Esc cancels");
    escape();
  });

  it.each(["select", "arrange"])(
    "keeps a tile under the pointer when the view zooms during the drag (%s)",
    async (pointer) => {
      sessionStorage.setItem("plenipo.orgPointer", pointer);
      const org = sampleOrganization();
      show(org);
      const layout = layoutOrganization(org);
      const node = layout.byId.get("p-sec")!;
      const auditor = await screen.findByRole("button", { name: "Security Auditor, Idle" });
      const start = centerOf(org, "p-sec");
      fireEvent.pointerDown(auditor, { pointerId: 1, button: 0, ...start });
      fireEvent.pointerMove(window, {
        pointerId: 1,
        clientX: start.clientX + 20,
        clientY: start.clientY + 20,
      });
      const before = zoom();
      fireEvent.wheel(canvas(), { deltaY: -200, clientX: 200, clientY: 200 });
      expect(zoom()).not.toBe(before);
      // An empty spot below everything, where it is on screen after the zoom.
      const spot = { x: node.x + node.w / 2 + 40, y: layout.bounds.y + layout.bounds.h + 300 };
      const to = toScreen(spot.x, spot.y);
      expect(toWorld(to).y).toBeCloseTo(spot.y);
      fireEvent.pointerMove(window, { pointerId: 1, ...to });
      fireEvent.pointerUp(window, { pointerId: 1, button: 0, ...to });
      await waitFor(() => expect(api.placeTiles).toHaveBeenCalledTimes(1));
      const [saved] = api.placeTiles.mock.calls[0]![0];
      // Its middle is where it was dropped.
      expect(saved!.x + node.w / 2).toBeCloseTo(spot.x, -1);
      expect(saved!.y + node.h / 2).toBeCloseTo(spot.y, -1);
    },
  );
});

describe("the pointer and the keys (ADR-053 §12)", () => {
  it("moves the view while the space bar is held, with a tile focused too", async () => {
    const org = sampleOrganization();
    show(org);
    const auditor = await screen.findByRole("button", { name: "Security Auditor, Idle" });
    auditor.focus();
    // The space bar does not press the focused tile: it holds the view.
    expect(fireEvent.keyDown(auditor, { key: " " })).toBe(false);
    expect(canvas()).toHaveClass("topology--pan");
    // Pressing another tile moves the focus to it; the view still moves.
    const reviewer = screen.getByRole("button", { name: "Code Reviewer, Idle" });
    const start = centerOf(org, "p-review");
    const before = document.querySelector<HTMLElement>(".topology__world")!.style.transform;
    fireEvent.pointerDown(reviewer, { pointerId: 1, button: 0, ...start });
    act(() => reviewer.focus());
    expect(canvas()).toHaveClass("topology--pan");
    fireEvent.pointerMove(window, {
      pointerId: 1,
      clientX: start.clientX + 40,
      clientY: start.clientY + 30,
    });
    expect(ghost()).toBeNull();
    fireEvent.pointerMove(window, {
      pointerId: 1,
      clientX: start.clientX + 80,
      clientY: start.clientY + 60,
    });
    expect(document.querySelector<HTMLElement>(".topology__world")!.style.transform).not.toBe(
      before,
    );
    fireEvent.pointerUp(window, { pointerId: 1, button: 0, clientX: 0, clientY: 0 });
    // What a press does is decided when pressed: letting go of the space bar before the pointer
    // moves still moves the view.
    fireEvent.pointerDown(reviewer, { pointerId: 1, button: 0, ...start });
    fireEvent.keyUp(reviewer, { key: " " });
    expect(canvas()).toHaveClass("topology--select");
    fireEvent.pointerMove(window, {
      pointerId: 1,
      clientX: start.clientX + 40,
      clientY: start.clientY + 30,
    });
    expect(ghost()).toBeNull();
    fireEvent.pointerUp(window, { pointerId: 1, button: 0, clientX: 0, clientY: 0 });
    await new Promise((r) => setTimeout(r, 20));
    expect(api.placeTiles).not.toHaveBeenCalled();
  });

  it("moves the view, not a line's end, in Move the view", async () => {
    const org = sampleOrganization();
    show(org);
    api.movePosition.mockResolvedValue(org);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Senior Developer, Working" }));
    const handle = screen.getByRole("button", { name: /^Line end: Senior Developer reports/ });
    await user.click(screen.getByRole("button", { name: "Move the view" }));
    const link = layoutOrganization(org).links.find((l) => l.childId === "p-dev")!;
    const target = centerOf(org, "p-camp");
    pointAt(handle, toScreen(link.from!.x, link.from!.y), target);
    expect(ghost()).toBeNull();
    fireEvent.pointerUp(window, { pointerId: 1, button: 0, ...target });
    expect(api.movePosition).not.toHaveBeenCalled();
  });

  it("a second finger puts back a tile being arranged", async () => {
    sessionStorage.setItem("plenipo.orgPointer", "arrange");
    const org = sampleOrganization();
    show(org);
    const node = layoutOrganization(org).byId.get("p-sec")!;
    const auditor = await screen.findByRole("button", { name: "Security Auditor, Idle" });
    const start = centerOf(org, "p-sec");
    fireEvent.pointerDown(auditor, { pointerId: 1, button: 0, ...start });
    fireEvent.pointerMove(window, {
      pointerId: 1,
      clientX: start.clientX + 60,
      clientY: start.clientY + 80,
    });
    await waitFor(() => expect(auditor.style.top).not.toBe(`${node.y}px`));
    fireEvent.pointerDown(canvas(), { pointerId: 2, button: 0, clientX: 20, clientY: 400 });
    await waitFor(() => expect(auditor.style.top).toBe(`${node.y}px`));
    fireEvent.pointerUp(window, { pointerId: 2, button: 0, clientX: 20, clientY: 400 });
    fireEvent.pointerUp(window, { pointerId: 1, button: 0, ...start });
    expect(api.placeTiles).not.toHaveBeenCalled();
  });

  it("Escape that cancels a drag, or closes the Add menu, keeps the details open", async () => {
    const org = sampleOrganization();
    show(org);
    const user = userEvent.setup();
    const reviewer = await screen.findByRole("button", { name: "Code Reviewer, Idle" });
    await user.click(reviewer);
    const details = () => screen.queryByRole("complementary", { name: "Details: Code Reviewer" });
    expect(details()).toBeInTheDocument();
    pointAt(reviewer, centerOf(org, "p-review"), { clientX: 30, clientY: 400 });
    expect(ghost()).not.toBeNull();
    fireEvent.keyDown(reviewer, { key: "Escape" });
    expect(ghost()).toBeNull();
    expect(details()).toBeInTheDocument();
    fireEvent.pointerUp(window, { pointerId: 1, button: 0, clientX: 30, clientY: 400 });

    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getByRole("menu")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(details()).toBeInTheDocument();
    // Escape on the canvas itself still closes them.
    fireEvent.keyDown(canvas(), { key: "Escape" });
    expect(details()).not.toBeInTheDocument();
  });

  it("lends an agent from its details, without a mouse (ADR-054)", async () => {
    const org = sampleOrganization();
    show(org);
    api.lendAgent.mockResolvedValue(org);
    const user = userEvent.setup();
    const auditor = await screen.findByRole("button", { name: "Security Auditor, Idle" });
    auditor.focus();
    await user.keyboard("{Enter}");
    const details = screen.getByRole("complementary", { name: "Details: Security Auditor" });
    await user.click(within(details).getByRole("tab", { name: "Team" }));
    const form = within(details).getByRole("form", { name: "Lend to another team" });
    await user.selectOptions(
      within(form).getByRole("combobox", { name: "Lend to the team of" }),
      "Campaign Supervisor",
    );
    await user.selectOptions(
      within(form).getByRole("combobox", { name: "For how long" }),
      "Until I send it home",
    );
    await user.click(within(form).getByRole("button", { name: "Lend" }));
    expect(api.lendAgent).toHaveBeenCalledWith("p-sec", "p-camp", "returned");
  });

  it("clears the filters to show an agent they hide", async () => {
    show();
    const user = userEvent.setup();
    await screen.findByRole("button", { name: "VP, Working" });
    await user.click(screen.getByRole("button", { name: "Filters" }));
    const panel = screen.getByRole("region", { name: "Filters" });
    await user.selectOptions(within(panel).getByRole("combobox", { name: "AI company" }), "OpenAI");
    expect(
      screen.queryByRole("button", { name: "Security Auditor, Idle" }),
    ).not.toBeInTheDocument();
    await user.type(
      screen.getByRole("searchbox", { name: "Find in the organization" }),
      "Security Auditor{Enter}",
    );
    expect(await screen.findByRole("button", { name: "Security Auditor, Idle" })).toBeVisible();
    expect(screen.getByRole("complementary", { name: "Details: Security Auditor" })).toBeVisible();
  });
});

describe("plain words on the canvas (ADR-010)", () => {
  it("names the filters' choices and the guide's buttons plainly", async () => {
    show();
    const user = userEvent.setup();
    await screen.findByRole("button", { name: "VP, Working" });
    await user.click(screen.getByRole("button", { name: "Filters" }));
    const panel = screen.getByRole("region", { name: "Filters" });
    for (const name of ["Any AI tool", "Any AI company", "Any department"]) {
      expect(within(panel).getByRole("option", { name })).toBeInTheDocument();
    }
    const guide = screen.getByRole("button", { name: "Guide to the canvas" });
    // There is no "?" key: the tooltip does not promise one.
    expect(guide).toHaveAttribute("title", "Guide to the canvas");
    await user.click(guide);
    const help = screen.getByRole("region", { name: "The canvas" });
    await user.click(within(help).getByRole("button", { name: "Close the guide" }));
    expect(screen.queryByRole("region", { name: "The canvas" })).not.toBeInTheDocument();
  });

  it("keys each part of a “where” line by its place, so two servers do not clash", async () => {
    const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
    api.getLiveView.mockResolvedValue({
      at: Date.now(),
      workers: [
        {
          grantId: "g1",
          taskId: "t1",
          positionId: "p-review",
          worker: "Code Reviewer",
          runtimeId: "claude-code",
          runsOn: { kind: "server", name: "Shop", production: false },
          touching: { kind: "server", name: "Shop", production: false },
        },
      ],
      handoffs: [],
    });
    show();
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Where" }));
    expect(await screen.findByText("Runs on Shop")).toBeInTheDocument();
    expect(screen.getByText("Touching Shop")).toBeInTheDocument();
    expect(errors.mock.calls.flat().join(" ")).not.toMatch(/same key/);
    errors.mockRestore();
  });
});

describe("Stop on a working tile (Phase 25, item 3.3)", () => {
  it("shows Stop only on tiles with work, asks first, and stops each of its tasks", async () => {
    api.cancelAgentTurn.mockResolvedValue({} as never);
    const user = userEvent.setup();
    show();
    // The VP works on its objective; the Senior Developer has one worker started, one queued.
    const vp = await screen.findByRole("button", { name: "Stop VP" });
    expect(screen.getByRole("button", { name: "Stop Senior Developer" })).toBeInTheDocument();
    // An idle tile has no Stop.
    expect(screen.queryByRole("button", { name: "Stop Engineering Manager" })).toBeNull();

    await user.click(vp);
    let dialog = screen.getByRole("dialog", { name: "Stop VP's task?" });
    expect(dialog).toHaveTextContent("Relaunch the website before the Q4 campaign");
    expect(dialog).toHaveTextContent("Its conversation stays");
    await user.click(within(dialog).getByRole("button", { name: "Stop" }));
    await waitFor(() => expect(api.cancelAgentTurn).toHaveBeenCalledWith("session-super"));

    // An on-call position stops the worker that started, not the one still queued.
    await user.click(screen.getByRole("button", { name: "Stop Senior Developer" }));
    dialog = screen.getByRole("dialog", { name: "Stop Senior Developer's task?" });
    expect(dialog).toHaveTextContent("Build the new pricing page");
    expect(dialog).not.toHaveTextContent("Migrate the blog");
    await user.click(within(dialog).getByRole("button", { name: "Stop" }));
    await waitFor(() => expect(api.cancelAgentTurn).toHaveBeenCalledWith("session-a-dev-1"));
    expect(api.cancelAgentTurn).toHaveBeenCalledTimes(2);
  });
});
