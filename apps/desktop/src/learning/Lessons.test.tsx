import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { LearningSnapshot, Lesson } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { LearningSwitch, NewLessons, RoleLessons } from "./Lessons";
import { useLearning } from "./useLearning";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getLearning: vi.fn(),
    setLearning: vi.fn(),
    setRoleLearning: vi.fn(),
    decideLesson: vi.fn(),
    removeLesson: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);

const lesson = (patch: Partial<Lesson> = {}): Lesson => ({
  id: "lesson-1",
  roleId: "r-web",
  taskId: "task-1",
  positionId: "p-web",
  worker: "Web Assistant",
  text: "The order number is on the Orders page.",
  state: "waiting",
  fromWeb: true,
  projectId: null,
  heldReason: null,
  createdAt: 0,
  decidedAt: null,
  decidedBy: null,
  ...patch,
});

const snapshot = (patch: Partial<LearningSnapshot> = {}): LearningSnapshot => ({
  enabled: true,
  autoRoles: [],
  offRoles: [],
  agents: {},
  waiting: [lesson()],
  kept: [lesson({ id: "lesson-0", state: "kept", text: "Sign in first.", fromWeb: false })],
  ...patch,
});

function Harness({ part }: { part: "new" | "role" | "switch" }) {
  const learning = useLearning();
  if (part === "new") return <NewLessons learning={learning} />;
  if (part === "role")
    return <RoleLessons learning={learning} roleId="r-web" roleName="Web Assistant" />;
  return <LearningSwitch learning={learning} />;
}

beforeEach(() => {
  api.getLearning.mockResolvedValue(snapshot());
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Learning", () => {
  it("keeps a new lesson in the owner's words, warning when it came from websites", async () => {
    const held = "Held for your review: it has a command, a path, or a web address.";
    api.getLearning.mockResolvedValue(snapshot({ waiting: [lesson({ heldReason: held })] }));
    api.decideLesson.mockResolvedValue(snapshot({ waiting: [] }));
    render(<Harness part="new" />);
    const card = await screen.findByRole("article", { name: "Lesson from Web Assistant" });
    expect(within(card).getByText("From a task that used websites or servers")).toBeInTheDocument();
    expect(within(card).getByText(held)).toBeInTheDocument();
    const box = within(card).getByRole("textbox");
    const user = userEvent.setup();
    await user.clear(box);
    await user.type(box, "Orders page has the number.");
    await user.click(within(card).getByRole("button", { name: "Keep" }));
    expect(api.decideLesson).toHaveBeenCalledWith("lesson-1", true, "Orders page has the number.");
    // Answered: the section goes away.
    await waitFor(() => expect(screen.queryByRole("heading", { name: "New lessons" })).toBeNull());
  });

  it("discards a lesson as written", async () => {
    api.decideLesson.mockResolvedValue(snapshot({ waiting: [] }));
    render(<Harness part="new" />);
    const card = await screen.findByRole("article", { name: "Lesson from Web Assistant" });
    await userEvent.setup().click(within(card).getByRole("button", { name: "Discard" }));
    expect(api.decideLesson).toHaveBeenCalledWith("lesson-1", false, undefined);
  });

  it("shows a role's lessons, removes one, and lets the role learn on its own", async () => {
    api.removeLesson.mockResolvedValue(snapshot({ kept: [] }));
    api.setRoleLearning.mockResolvedValue(snapshot({ kept: [], autoRoles: ["r-web"] }));
    render(<Harness part="role" />);
    expect(await screen.findByText(/Sign in first\./)).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Remove" }));
    expect(api.removeLesson).toHaveBeenCalledWith("lesson-0");
    const own = await screen.findByRole("switch", { name: "Learn on its own" });
    expect(own).toHaveAttribute("aria-checked", "false");
    await user.click(own);
    expect(api.setRoleLearning).toHaveBeenCalledWith("r-web", true);
    expect(await screen.findByRole("switch", { name: "Learn on its own" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  it("switches worker learning off in Settings", async () => {
    api.setLearning.mockResolvedValue(snapshot({ enabled: false }));
    render(<Harness part="switch" />);
    const toggle = await screen.findByRole("switch", { name: "Worker learning" });
    await screen.findByText("On");
    await userEvent.setup().click(toggle);
    expect(api.setLearning).toHaveBeenCalledWith(false);
    expect(await screen.findByRole("switch", { name: "Worker learning" })).toHaveAttribute(
      "aria-checked",
      "false",
    );
  });
});
