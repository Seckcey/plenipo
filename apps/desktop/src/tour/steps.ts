/**
 * The setup tour's steps (Phase 25, item 2.9; ADR-198): each one points at a part of a page,
 * says what to do there in plain words, and moves on only once that is really done. Pure: the
 * facts come from the AI tools, the organization, and the model settings.
 */
import type { OrgSnapshot } from "@plenipo/types";

import type { Place } from "../components/views";

/** What the tour knows about this organization now. */
export interface TourFacts {
  /** AI tools you sign in to (not paid per use) that are ready. */
  signedIn: number;
  /** Departments that are active. */
  departments: number;
  projects: number;
  /** Workers (not leads) in the organization. */
  workers: number;
  /** Model choices you changed since the tour came to the models step. */
  modelChanges: number;
  /** Objectives going, waiting, or finished today. */
  objectives: number;
}

export type StepId =
  | "welcome"
  | "aiTools"
  | "organization"
  | "department"
  | "project"
  | "team"
  | "models"
  | "objective"
  | "watch";

export interface SetupStep {
  id: StepId;
  title: string;
  words: string;
  /** The page it shows (`null`: the page you're on). */
  page: Place | null;
  /**
   * The parts of the page it points at (`data-tour` marks), the first one on screen wins; none
   * (or none showing yet): the middle of the screen.
   */
  anchors: readonly string[];
  /** Bring the first project's supervisor into view on the map. */
  focusSupervisor?: boolean;
  /**
   * Whether the step is done. `null`: done when you press Next (nothing to check), which is also
   * how a step you choose to skip ends.
   */
  done: ((f: TourFacts) => boolean) | null;
}

export const SETUP_STEPS: readonly SetupStep[] = [
  {
    id: "welcome",
    title: "Welcome to Plenipo",
    words:
      "This tour sets up your first team, step by step: sign in to an AI tool, add a department and a project, hire the team, choose their models, and give them their first objective. Each step moves on when it's done. You can stop at any time and pick up where you left off.",
    page: { view: "home", id: null },
    anchors: [],
    done: null,
  },
  {
    id: "aiTools",
    title: "Sign in to an AI tool",
    words:
      "Your workers run on your own AI subscriptions. Open a card and press Sign in: a tab opens at the bottom where you sign in yourself. This step moves on once a card's light is green.",
    page: { view: "runtimes", id: null },
    anchors: ["ai-tools"],
    done: (f) => f.signedIn > 0,
  },
  {
    id: "organization",
    title: "Name your organization, or pick a template",
    words:
      "Give your organization a name here. A template adds whole departments with their teams in one go; Software project is a good start. Press Next when you're happy with it.",
    page: { view: "settings", id: "organization" },
    anchors: ["organization"],
    done: null,
  },
  {
    id: "department",
    title: "Add a department",
    words:
      "A department is a part of your organization with its own manager, such as Development. Open the hire palette on the left and press + Department, or pick a department template.",
    page: { view: "organization", id: null },
    anchors: ["add-department", "hire", "add-menu"],
    done: (f) => f.departments > 0,
  },
  {
    id: "project",
    title: "Set up your first project",
    words:
      "A project is one piece of work with its own folder, such as your website. Press + Project, or use the Software project template for a ready-made team.",
    page: { view: "organization", id: null },
    anchors: ["add-project", "hire", "add-menu"],
    done: (f) => f.projects > 0,
  },
  {
    id: "team",
    title: "Hire the team",
    words:
      "Drag a role from the hire palette onto your project's supervisor, or click it. A template already brought one: Plenipo uses the workers you have before hiring new ones.",
    page: { view: "organization", id: null },
    anchors: ["hire"],
    focusSupervisor: true,
    done: (f) => f.workers > 0,
  },
  {
    id: "models",
    title: "Choose a model for each job",
    words:
      "Here you pick the exact model each role uses, for example Fable for the Senior Developer and Sonnet for the Documentation Writer. Press Change on a row. Or press Next to let each role choose.",
    page: { view: "settings", id: "aiModels" },
    anchors: ["who-uses-what"],
    done: (f) => f.modelChanges > 0,
  },
  {
    id: "objective",
    title: "Give the first objective",
    words:
      "Your project's supervisor is selected on the map. Press Give an objective and say what you want done, in a few sentences; the supervisor hands the parts to its team.",
    page: { view: "organization", id: null },
    anchors: ["give-objective"],
    focusSupervisor: true,
    done: (f) => f.objectives > 0,
  },
  {
    id: "watch",
    title: "Watch it work",
    words:
      "Press Watch on a working tile to see each file its team changes, line by line. You're set up: Home shows what's going, what's stuck, and what's waiting for you.",
    page: { view: "organization", id: null },
    anchors: ["watch"],
    focusSupervisor: true,
    done: null,
  },
];

/** The first step from `from` on that isn't done yet (the last step when all are). */
export function nextStep(facts: TourFacts, from: number): number {
  for (let i = Math.max(0, from); i < SETUP_STEPS.length; i++) {
    const done = SETUP_STEPS[i]?.done;
    if (!done || !done(facts)) return i;
  }
  return SETUP_STEPS.length - 1;
}

/**
 * Whether the tour starts by itself in this organization: never toured, no project yet, and
 * either nothing set up or made in the last day (a template adds departments, not projects).
 */
export function startsByItself(
  facts: TourFacts,
  { seen, recent }: { seen: boolean; recent: boolean },
): boolean {
  return !seen && facts.projects === 0 && (facts.departments === 0 || recent);
}

/** What the tour knows from the organization (`modelChanges` is counted by the tour itself). */
export function factsOf(org: OrgSnapshot, modelChanges = 0): TourFacts {
  const s = org.stats;
  return {
    signedIn: org.runtimes.filter((r) => r.ready && !r.paid).length,
    departments: org.departments.filter((d) => d.active).length,
    projects: org.projects.filter((p) => p.active).length,
    workers: org.positions.filter((p) => p.active && !p.deleted && p.kind === "worker").length,
    modelChanges,
    objectives: s.working + s.waiting + s.queued + s.completed24h + s.failed24h,
  };
}

/** The supervisor of the first active project, to bring into view on the map. */
export function firstSupervisor(org: OrgSnapshot): string | null {
  const project = org.projects.find((p) => p.active && p.coordinatorPositionId);
  return project?.coordinatorPositionId ?? null;
}
