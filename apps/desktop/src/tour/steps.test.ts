import { describe, expect, it } from "vitest";

import { emptyOrganization, sampleOrganization } from "../test/orgFixtures";
import {
  SETUP_STEPS,
  factsOf,
  firstSupervisor,
  nextStep,
  startsByItself,
  type StepId,
  type TourFacts,
} from "./steps";

/** Nothing set up yet. */
const NOTHING: TourFacts = {
  signedIn: 0,
  departments: 0,
  projects: 0,
  workers: 0,
  modelChanges: 0,
  objectives: 0,
};

const stepOf = (id: StepId) => {
  const step = SETUP_STEPS.find((s) => s.id === id);
  if (!step) throw new Error(`no step ${id}`);
  return step;
};

const indexOf = (id: StepId) => SETUP_STEPS.findIndex((s) => s.id === id);

describe("the setup tour's steps (Phase 25, item 2.9)", () => {
  it("goes in the owner's order", () => {
    expect(SETUP_STEPS.map((s) => s.id)).toEqual([
      "welcome",
      "aiTools",
      "organization",
      "department",
      "project",
      "team",
      "models",
      "objective",
      "watch",
    ]);
  });

  it.each<[StepId, Partial<TourFacts>]>([
    ["aiTools", { signedIn: 1 }],
    ["department", { departments: 1 }],
    ["project", { projects: 1 }],
    ["team", { workers: 1 }],
    ["models", { modelChanges: 1 }],
    ["objective", { objectives: 1 }],
  ])("%s is done only once it's really done", (id, done) => {
    const check = stepOf(id).done;
    expect(check).not.toBeNull();
    expect(check?.(NOTHING)).toBe(false);
    expect(check?.({ ...NOTHING, ...done })).toBe(true);
  });

  it.each<StepId>(["welcome", "organization", "watch"])(
    "%s has nothing to check: it ends with Next",
    (id) => {
      expect(stepOf(id).done).toBeNull();
    },
  );

  it("points each step at its page", () => {
    expect(stepOf("aiTools").page).toEqual({ view: "runtimes", id: null });
    expect(stepOf("organization").page).toEqual({ view: "settings", id: "organization" });
    expect(stepOf("models").page).toEqual({ view: "settings", id: "aiModels" });
    expect(stepOf("department").anchors[0]).toBe("add-department");
    expect(stepOf("objective").focusSupervisor).toBe(true);
  });

  it("moves past the steps already done, and stops at one with nothing to check", () => {
    // Signed in already: from the AI tools step, the next one is naming the organization.
    expect(nextStep({ ...NOTHING, signedIn: 1 }, indexOf("aiTools"))).toBe(indexOf("organization"));
    // A template brought a department and its team: on to the project.
    expect(
      nextStep({ ...NOTHING, signedIn: 1, departments: 1, workers: 3 }, indexOf("department")),
    ).toBe(indexOf("project"));
    // Nothing done: it stays.
    expect(nextStep(NOTHING, indexOf("department"))).toBe(indexOf("department"));
    // Welcome always shows first.
    expect(nextStep({ ...NOTHING, signedIn: 2 }, 0)).toBe(0);
    // Past the end: the last step.
    expect(nextStep(NOTHING, SETUP_STEPS.length + 3)).toBe(SETUP_STEPS.length - 1);
  });

  it("starts by itself only in a new organization never toured", () => {
    expect(startsByItself(NOTHING, { seen: false, recent: false })).toBe(true);
    expect(startsByItself(NOTHING, { seen: true, recent: true })).toBe(false);
    // A template's departments, just made: yes. An older organization with departments: no.
    const departments = { ...NOTHING, departments: 2 };
    expect(startsByItself(departments, { seen: false, recent: true })).toBe(true);
    expect(startsByItself(departments, { seen: false, recent: false })).toBe(false);
    // A project already: no.
    expect(startsByItself({ ...NOTHING, projects: 1 }, { seen: false, recent: true })).toBe(false);
  });

  it("reads its facts from the organization", () => {
    const blank = emptyOrganization();
    const s = blank.stats;
    // Claude Code is signed in (a subscription); Codex is not. Objectives count today's work.
    expect(factsOf(blank)).toEqual({
      ...NOTHING,
      signedIn: 1,
      objectives: s.working + s.waiting + s.queued + s.completed24h + s.failed24h,
    });
    const org = sampleOrganization();
    const facts = factsOf(org, 2);
    expect(facts.departments).toBe(org.departments.filter((d) => d.active).length);
    expect(facts.projects).toBe(org.projects.filter((p) => p.active).length);
    expect(facts.workers).toBeGreaterThan(0);
    expect(facts.modelChanges).toBe(2);
    // A key paid per use doesn't count as signed in.
    const paid = emptyOrganization();
    paid.runtimes = [
      { id: "openrouter", label: "OpenRouter", ready: true, company: "OpenRouter", paid: true },
    ];
    expect(factsOf(paid).signedIn).toBe(0);
  });

  it("finds the first project's supervisor to bring into view", () => {
    const org = sampleOrganization();
    const project = org.projects.find((p) => p.active && p.coordinatorPositionId);
    expect(firstSupervisor(org)).toBe(project?.coordinatorPositionId ?? null);
    expect(firstSupervisor(emptyOrganization())).toBeNull();
  });
});
