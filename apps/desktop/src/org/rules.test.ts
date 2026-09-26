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
  it("lets leaders report to the owner or a VP only", () => {
    expect(moveRefusal(org, at("p-eng"), null)).toBeNull();
    expect(moveRefusal(org, at("p-eng"), "p-camp")).toMatch(/^A Manager reports to you or to a VP/);
    expect(moveRefusal(org, at("p-eng"), "p-web")).toMatch(/cannot report to it/);
    expect(hireRefusal(org, roleNamed("VP"), null)).toBeNull();
    expect(hireRefusal(org, roleNamed("VP"), "p-eng")).toMatch(/^A VP reports to you or to a VP/);
  });

  it("keeps supervisors under department managers and workers under full-time leads", () => {
    expect(moveRefusal(org, at("p-web"), "p-mkt")).toBeNull();
    expect(moveRefusal(org, at("p-web"), "p-super")).toMatch(
      /^A Supervisor reports to the Manager of a department/,
    );
    expect(moveRefusal(org, at("p-dev"), "p-camp")).toBeNull();
    expect(moveRefusal(org, at("p-dev"), null)).toMatch(/not directly to you/);
    expect(moveRefusal(org, at("p-dev"), "p-review")).toMatch(/on-call position/);
  });

  it("uses the organization's chosen titles", () => {
    const army = { ...org, titles: "army" as const };
    expect(moveRefusal(army, at("p-web"), "p-super")).toBe(
      "A Sergeant reports to the Captain of a department.",
    );
    expect(moveRefusal(army, at("p-eng"), "p-camp")).toBe(
      "A Captain reports to you or to a Colonel.",
    );
    const mafia = { ...org, titles: "mafia" as const };
    expect(moveRefusal(mafia, at("p-dev"), null)).toBe(
      "An Associate reports to a full-time position on a team, such as a Soldier, not directly to you.",
    );
    expect(hireRefusal(mafia, roleNamed("Manager"), null)).toBe(
      "A Capo comes with a department: create a department instead.",
    );
  });

  it("refuses cycles and no-op moves", () => {
    expect(moveRefusal(org, at("p-eng"), "p-eng")).toMatch(/Already|itself/);
    expect(moveRefusal(org, at("p-super"), "p-eng")).toMatch(/reports to VP/);
    expect(moveRefusal(org, at("p-dev"), "p-web")).toMatch(/Already reports/);
  });

  it("offers every valid supervisor", () => {
    expect(moveChoices(org, at("p-dev"))).toEqual(["p-super", "p-eng", "p-mkt", "p-camp"]);
    expect(supervisorChoices(org, roleNamed("VP"))).toEqual([null, "p-super"]);
    expect(hireableRoles(org).map((r) => r.kind)).not.toContain("departmentManager");
    expect(hireRefusal(org, roleNamed("Manager"), null)).toMatch(/create a department/);
    expect(hireRefusal(org, roleNamed("Supervisor"), "p-eng")).toMatch(/create a project/);
  });

  it("lets on-call positions oversee full-time leads' teams", () => {
    expect(oversightRefusal(org, at("p-review"), at("p-camp"), "review")).toBeNull();
    expect(oversightRefusal(org, at("p-web"), at("p-camp"), "review")).toMatch(
      /full-time position/,
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

  it("gives objectives only to staffed full-time positions", () => {
    expect(canTakeObjective(at("p-super"))).toBe(true);
    expect(canTakeObjective(at("p-mkt"))).toBe(false);
    expect(canTakeObjective(at("p-dev"))).toBe(false);
  });

  it("starts new positions on a ready AI tool the project allows", () => {
    expect(defaultRuntime(org, null)).toBe("claude-code");
    expect(defaultRuntime(org, ["codex"])).toBe("codex");
    expect(defaultRuntime(org, [])).toBe("claude-code");
  });
});
