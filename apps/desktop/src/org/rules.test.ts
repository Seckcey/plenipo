import { describe, expect, it } from "vitest";

import { ROLES, sampleOrganization } from "../test/orgFixtures";
import {
  canTakeObjective,
  defaultRuntime,
  hireRefusal,
  hireableRoles,
  moveChoices,
  moveRefusal,
  oversightOrder,
  oversightRefusal,
  positionMap,
  supervisorChoices,
} from "./rules";

const org = sampleOrganization();
const at = (id: string) => positionMap(org).get(id)!;
const roleNamed = (name: string) => ROLES.find((r) => r.name === name)!;

describe("structure hints (mirroring the Ledger's rules)", () => {
  it("lets leaders report to the owner or a superintendent only", () => {
    expect(moveRefusal(org, at("p-eng"), null)).toBeNull();
    expect(moveRefusal(org, at("p-eng"), "p-camp")).toMatch(
      /reports to you or to a superintendent/,
    );
    expect(moveRefusal(org, at("p-eng"), "p-web")).toMatch(/cannot report to it/);
    expect(hireRefusal(org, roleNamed("Superintendent"), null)).toBeNull();
    expect(hireRefusal(org, roleNamed("Superintendent"), "p-eng")).toMatch(/superintendent/);
  });

  it("keeps coordinators under department heads and workers under persistent leads", () => {
    expect(moveRefusal(org, at("p-web"), "p-mkt")).toBeNull();
    expect(moveRefusal(org, at("p-web"), "p-super")).toMatch(/head of a department/);
    expect(moveRefusal(org, at("p-dev"), "p-camp")).toBeNull();
    expect(moveRefusal(org, at("p-dev"), null)).toMatch(/not directly to you/);
    expect(moveRefusal(org, at("p-dev"), "p-review")).toMatch(/on-demand position/);
  });

  it("refuses cycles and no-op moves", () => {
    expect(moveRefusal(org, at("p-eng"), "p-eng")).toMatch(/Already|itself/);
    expect(moveRefusal(org, at("p-super"), "p-eng")).toMatch(/reports to Superintendent/);
    expect(moveRefusal(org, at("p-dev"), "p-web")).toMatch(/Already reports/);
  });

  it("offers every valid supervisor", () => {
    expect(moveChoices(org, at("p-dev"))).toEqual(["p-super", "p-eng", "p-mkt", "p-camp"]);
    expect(supervisorChoices(org, roleNamed("Superintendent"))).toEqual([null, "p-super"]);
    expect(hireableRoles(org).map((r) => r.kind)).not.toContain("departmentManager");
    expect(hireRefusal(org, roleNamed("Department Manager"), null)).toMatch(/create a department/);
    expect(hireRefusal(org, roleNamed("Project Coordinator"), "p-eng")).toMatch(/create a project/);
  });

  it("lets on-demand positions oversee persistent leads' teams", () => {
    expect(oversightRefusal(org, at("p-review"), at("p-camp"), "review")).toBeNull();
    expect(oversightRefusal(org, at("p-web"), at("p-camp"), "review")).toMatch(
      /persistent position/,
    );
    expect(oversightRefusal(org, at("p-review"), at("p-dev"), "qa")).toMatch(/no team to oversee/);
    expect(oversightRefusal(org, at("p-review"), at("p-web"), "qa")).toMatch(/already on/);
    expect(oversightRefusal(org, at("p-sec"), at("p-web"), "security")).toMatch(
      /already the security auditor/,
    );
    expect(oversightRefusal(org, at("p-sec"), at("p-web"), "review")).toBeNull();
    expect(oversightOrder("shield")[0]).toBe("security");
    expect(oversightOrder("qa")[0]).toBe("qa");
    expect(oversightOrder("code")).toEqual(["review", "qa", "security"]);
  });

  it("gives objectives only to staffed persistent positions", () => {
    expect(canTakeObjective(at("p-super"))).toBe(true);
    expect(canTakeObjective(at("p-mkt"))).toBe(false);
    expect(canTakeObjective(at("p-dev"))).toBe(false);
  });

  it("starts new positions on a ready runtime the project allows", () => {
    expect(defaultRuntime(org, null)).toBe("claude-code");
    expect(defaultRuntime(org, ["codex"])).toBe("codex");
    expect(defaultRuntime(org, [])).toBe("claude-code");
  });
});
