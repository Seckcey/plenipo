import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PhoneAsk } from "@plenipo/types";

import { App } from "./App";
import { memoryKeep, type Kept } from "./keep";
import { newKeyPair } from "./lock/noise";
import { encode } from "./lock/bytes";
import { FakePc } from "./test/fake-pc";
import type { PhoneAiTool } from "./words";

// The phone's keychain: a stand-in (jsdom has no passkeys). The PC checks real answers in Rust.
vi.mock("./lock/passkey", () => ({
  PasskeyProblem: class PasskeyProblem extends Error {},
  makePasskey: vi.fn(() =>
    Promise.resolve({
      id: "Y3JlZGVudGlhbA",
      publicKey: "AA",
      algorithm: -7,
      authenticatorData: "AA",
      clientData: "AA",
    }),
  ),
  answerChallenge: vi.fn(() =>
    Promise.resolve({
      id: "Y3JlZGVudGlhbA",
      authenticatorData: "AA",
      clientData: "AA",
      signature: "AA",
    }),
  ),
}));

const home = {
  current: [
    {
      rootTaskId: "t1",
      objective: "Make the website faster",
      positionTitle: "Development Manager",
      state: "running",
      createdAt: Date.now() - 60_000,
      completedAt: null,
      tasks: 3,
      active: 1,
      failed: 0,
      waitingApprovals: 1,
      branch: null,
      projectId: null,
      answer: null,
    },
  ],
  finished: [],
  stuck: [],
  going: 1,
  finishedDay: 0,
};

const approval = {
  id: "a1",
  taskId: "t1",
  status: "pending",
  requestedAt: Date.now() - 10_000,
  expiresAt: Date.now() + 9 * 60_000,
  resolvedAt: null,
  worker: "Backend Developer",
  role: "Developer",
  capabilityLabel: "Run programs",
  summary: "git push to Website",
  detail: "git push origin main",
  reason: "Publishing changes outside this computer.",
  riskLabel: "Sends outside",
  waiting: true,
};

async function pairedWith(pc: FakePc): Promise<Kept> {
  const keys = await newKeyPair(true);
  pc.phones.set("cGhvbmUtMQ", keys.publicKey);
  return {
    keys,
    paired: {
      device: "ZGV2aWNlLTE",
      phone: "cGhvbmUtMQ",
      pass: "cGhvbmUtMQ.pass",
      pc: pc.fingerprint,
      pcKey: encode(pc.keys.publicKey),
      pcName: pc.pcName,
      credential: "Y3JlZGVudGlhbA",
    },
  };
}

let pc: FakePc;

beforeEach(async () => {
  localStorage.clear();
  window.location.hash = "";
  pc = await FakePc.start();
  pc.answer = (ask) => {
    switch (ask.kind) {
      case "readOrganizations":
        return { organizations: [{ id: "first", name: "8 West Ventures" }], pcName: pc.pcName };
      case "readHome":
        return home;
      case "readApprovals":
        return [
          {
            org: "first",
            name: "8 West Ventures",
            queue: {
              pending: pc.asked.some((a) => a.kind === "approve") ? [] : [approval],
              recent: [],
            },
            keptOnPc: [],
          },
        ];
      case "approve":
        return { ...approval, status: "approved" };
      case "readLessons":
        return { enabled: true, autoRoles: [], offRoles: [], agents: {}, waiting: [], kept: [] };
      case "readAiTools":
        return {
          tools: [
            {
              runtimeId: "codex",
              outOfService: "Plenipo gives Codex no tasks: its update failed.",
            },
          ],
          autoUpdate: false,
          lastLookedAt: null,
          looking: false,
          runtimes: [
            aiTool("claude-code", "Claude Code", { ready: true }),
            aiTool("codex", "Codex", { ready: true }),
            aiTool("grok", "Grok Build", { auth: "signedOut" }),
            aiTool("kimi", "Kimi Code", { install: "notInstalled", auth: "unknown" }),
          ],
        };
      case "readDiagnostics":
        return {
          ledger: { taskCount: 3, eventCount: 40, lastBackup: null },
          version: "1.19.0",
        };
      default:
        return {};
    }
  };
});

/** An AI tool as the PC sends it to the phone. */
function aiTool(id: string, label: string, state: Partial<PhoneAiTool> = {}): PhoneAiTool {
  return {
    id,
    label,
    ready: false,
    install: "installed",
    auth: "subscription",
    held: null,
    ...state,
  };
}

describe("pairing this phone", () => {
  it("pairs with the typed code, after the PC's owner says yes", async () => {
    const keep = memoryKeep();
    const user = userEvent.setup();
    render(<App keep={keep} make={pc.make} />);
    const code = await screen.findByLabelText("Or type the code");
    await user.type(code, "7k3q-m9tx-2hfd-r8wb");
    await user.clear(screen.getByLabelText("What to call this phone"));
    await user.type(screen.getByLabelText("What to call this phone"), "Frank's phone");
    await user.click(screen.getByRole("button", { name: "Pair this phone" }));
    await user.click(
      await screen.findByRole("button", { name: "Set up Face ID, fingerprint, or passcode" }),
    );
    // Paired, signed in, and on Home.
    expect(await screen.findByText("Make the website faster")).toBeInTheDocument();
    expect(pc.hello?.name).toBe("Frank's phone");
    expect(keep.kept?.paired.pcName).toBe("Office PC");
    // The phone's own key can never be copied out.
    expect(keep.kept?.keys.privateKey.extractable).toBe(false);
  });

  it("says plainly when a code does not work, and to look at the PC", async () => {
    // A closed mailbox means the code was mistyped, ran out, or was used already. In the last
    // case the PC may be asking about a stranger's phone (ADR-212), so the page says to look.
    const user = userEvent.setup();
    render(<App keep={memoryKeep()} make={pc.make} />);
    await user.type(await screen.findByLabelText("Or type the code"), "0000-0000-0000-0000");
    await user.click(screen.getByRole("button", { name: "Pair this phone" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/This code was already used/);
    expect(alert).toHaveTextContent(
      /Look at your PC: if it is asking about a phone that is not yours, click Cancel/,
    );
  });

  it("shows the six digits your PC shows too, while your PC asks (ADR-212)", async () => {
    pc.ownerSays = "wait";
    const user = userEvent.setup();
    render(<App keep={memoryKeep()} make={pc.make} />);
    await user.type(await screen.findByLabelText("Or type the code"), pc.code);
    await user.click(screen.getByRole("button", { name: "Pair this phone" }));
    const asking = (await screen.findByText(/Is this your phone/)).closest("p")!;
    await waitFor(() => expect(pc.check).not.toBeNull());
    // The same six digits the PC made from the same meeting, shown as two groups of three.
    expect(asking).toHaveTextContent(`${pc.check!.slice(0, 3)} ${pc.check!.slice(3)}`);
    expect(asking).toHaveTextContent(/same six digits/);
    expect(asking).toHaveTextContent(/Cancel/);
  });

  it("says why when another phone used the code first", async () => {
    pc.codeUsed = true;
    const keep = memoryKeep();
    const user = userEvent.setup();
    render(<App keep={keep} make={pc.make} />);
    await user.type(await screen.findByLabelText("Or type the code"), pc.code);
    await user.click(screen.getByRole("button", { name: "Pair this phone" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Another phone already used this code.",
    );
    expect(keep.kept).toBeNull();
  });

  it("adds nothing when the PC's owner says no", async () => {
    pc.ownerSays = "no";
    const keep = memoryKeep();
    const user = userEvent.setup();
    render(<App keep={keep} make={pc.make} />);
    await user.type(await screen.findByLabelText("Or type the code"), pc.code);
    await user.click(screen.getByRole("button", { name: "Pair this phone" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Your PC said this is not your phone.",
    );
    expect(keep.kept).toBeNull();
  });

  it("only offers pairing for a whole code", async () => {
    render(<App keep={memoryKeep()} make={pc.make} />);
    await userEvent.type(await screen.findByLabelText("Or type the code"), "7K3Q");
    expect(screen.getByRole("button", { name: "Pair this phone" })).toBeDisabled();
  });
});

describe("a paired phone", () => {
  it("signs in with its passkey, then reads Home", async () => {
    const kept = await pairedWith(pc);
    const keep = memoryKeep(kept);
    const user = userEvent.setup();
    render(<App keep={keep} make={pc.make} />);
    await user.click(await screen.findByRole("button", { name: "Check it’s you" }));
    expect(await screen.findByText("Make the website faster")).toBeInTheDocument();
    expect(screen.getByText(/Waiting for you: 1 approval/)).toBeInTheDocument();
    // The PC's fresh pass is kept.
    expect(keep.kept?.paired.pass).toBe("cGhvbmUtMQ.renewed");
  });

  it("approves from the phone, and the PC records it", async () => {
    pc.signedIn.add("cGhvbmUtMQ");
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await pairedWith(pc))} make={pc.make} />);
    await user.click(await screen.findByRole("button", { name: "Approvals" }));
    const card = (await screen.findByText("git push to Website")).closest("li")!;
    expect(within(card).getByText("git push origin main")).toBeInTheDocument();
    await user.click(within(card).getByRole("button", { name: "Approve" }));
    // Answered: the PC has it, and nothing is left waiting.
    await waitFor(() =>
      expect(pc.asked).toContainEqual({ kind: "approve", org: "first", approval: "a1" }),
    );
    expect(await screen.findByText("Nothing is waiting for you.")).toBeInTheDocument();
    expect(pc.asked).toContainEqual({ kind: "approve", org: "first", approval: "a1" });
  });

  it("works from the keyboard alone", async () => {
    pc.signedIn.add("cGhvbmUtMQ");
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await pairedWith(pc))} make={pc.make} />);
    await screen.findByText("Make the website faster");
    const tabTo = async (target: HTMLElement) => {
      for (let i = 0; i < 40 && document.activeElement !== target; i++) await user.tab();
      expect(target).toHaveFocus();
    };
    await tabTo(screen.getByRole("button", { name: "Approvals" }));
    await user.keyboard("{Enter}");
    const approve = await screen.findByRole("button", { name: "Approve" });
    await tabTo(approve);
    await user.keyboard("{Enter}");
    // Answered: the PC has it, and nothing is left waiting.
    await waitFor(() =>
      expect(pc.asked).toContainEqual({ kind: "approve", org: "first", approval: "a1" }),
    );
    expect(await screen.findByText("Nothing is waiting for you.")).toBeInTheDocument();
    expect(pc.asked).toContainEqual({ kind: "approve", org: "first", approval: "a1" });
  });

  it("shows an approval kept on the PC with no buttons", async () => {
    pc.signedIn.add("cGhvbmUtMQ");
    const answer = pc.answer;
    pc.answer = (ask) =>
      ask.kind === "readApprovals"
        ? [
            {
              org: "first",
              name: "8 West",
              queue: { pending: [approval], recent: [] },
              keptOnPc: ["a1"],
            },
          ]
        : answer(ask);
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await pairedWith(pc))} make={pc.make} />);
    await user.click(await screen.findByRole("button", { name: "Approvals" }));
    const card = (await screen.findByText("git push to Website")).closest("li")!;
    expect(within(card).getByText(/Approve on your PC/)).toBeInTheDocument();
    expect(within(card).queryByRole("button", { name: "Approve" })).toBeNull();
  });

  it("reads again when the PC says something changed", async () => {
    pc.signedIn.add("cGhvbmUtMQ");
    render(<App keep={memoryKeep(await pairedWith(pc))} make={pc.make} />);
    await screen.findByText("Make the website faster");
    const before = pc.asked.filter((a) => a.kind === "readHome").length;
    await pc.tell("tasks");
    await waitFor(() =>
      expect(pc.asked.filter((a) => a.kind === "readHome").length).toBeGreaterThan(before),
    );
  });

  it("when the PC can't be reached, says so and changes nothing", async () => {
    pc.signedIn.add("cGhvbmUtMQ");
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await pairedWith(pc))} make={pc.make} />);
    await screen.findByText("Make the website faster");
    pc.goOffline();
    expect(
      await screen.findByText("Your PC can’t be reached. Nothing was changed."),
    ).toBeInTheDocument();
    pc.online = true;
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Make the website faster")).toBeInTheDocument();
  });

  it("switched off on the PC: the phone hears it, then says the PC can't be reached", async () => {
    pc.signedIn.add("cGhvbmUtMQ");
    render(<App keep={memoryKeep(await pairedWith(pc))} make={pc.make} />);
    await screen.findByText("Make the website faster");
    await pc.switchOff();
    expect(
      await screen.findByText("Your PC can’t be reached. Nothing was changed."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Check it’s you" })).toBeNull();
  });

  it("removed just before the line closes: the phone still forgets the PC", async () => {
    pc.signedIn.add("cGhvbmUtMQ");
    const keep = memoryKeep(await pairedWith(pc));
    render(<App keep={keep} make={pc.make} />);
    await screen.findByText("Make the website faster");
    await pc.removeAndClose();
    expect(await screen.findByRole("heading", { name: "Pair this phone" })).toBeInTheDocument();
    expect(keep.kept).toBeNull();
  });

  it("a removed phone is told, and can be paired again", async () => {
    pc.removed.add("cGhvbmUtMQ");
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await pairedWith(pc))} make={pc.make} />);
    expect(
      await screen.findByRole("heading", { name: "This phone is no longer on your PC’s list" }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Pair this phone again" }));
    expect(await screen.findByRole("heading", { name: "Pair this phone" })).toBeInTheDocument();
  });

  it("a phone removed while it was away is told so when it comes back", async () => {
    pc.forgotten.add("cGhvbmUtMQ");
    render(<App keep={memoryKeep(await pairedWith(pc))} make={pc.make} />);
    expect(
      await screen.findByRole("heading", { name: "This phone is no longer on your PC’s list" }),
    ).toBeInTheDocument();
  });

  it("Stop all asks first, then stops", async () => {
    pc.signedIn.add("cGhvbmUtMQ");
    let stopped = false;
    const answer = pc.answer;
    pc.answer = (ask) =>
      ask.kind === "stopAll"
        ? ((stopped = true), { stopped: true, sessions: [], revision: 1 })
        : ask.kind === "readControl"
          ? { control: { stopped, sessions: [], revision: 1 }, recovery: [] }
          : answer(ask);
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await pairedWith(pc))} make={pc.make} />);
    await screen.findByText("Make the website faster");
    await user.click(screen.getByRole("button", { name: "Stop all" }));
    const ask = screen.getByRole("alertdialog", { name: "Stop all?" });
    await user.click(within(ask).getByRole("button", { name: "Stop all" }));
    expect(
      await screen.findByText("Browser, desktop, and server work is stopped."),
    ).toBeInTheDocument();
    expect(pc.asked).toContainEqual({ kind: "stopAll" });
  });

  it("removing this phone asks first, then forgets it", async () => {
    pc.signedIn.add("cGhvbmUtMQ");
    const keep = memoryKeep(await pairedWith(pc));
    const user = userEvent.setup();
    render(<App keep={keep} make={pc.make} />);
    await screen.findByText("Make the website faster");
    await user.click(screen.getByRole("button", { name: "More" }));
    await user.click(screen.getByRole("button", { name: "Remove this phone" }));
    const ask = screen.getByRole("alertdialog", { name: "Remove this phone?" });
    await user.click(within(ask).getByRole("button", { name: "Remove this phone" }));
    expect(await screen.findByRole("heading", { name: "Pair this phone" })).toBeInTheDocument();
    expect(keep.kept).toBeNull();
    expect(pc.asked).toContainEqual({ kind: "removeThisPhone" });
  });

  it("More names each AI tool on the PC, with what it can do now", async () => {
    pc.signedIn.add("cGhvbmUtMQ");
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await pairedWith(pc))} make={pc.make} />);
    await user.click(await screen.findByRole("button", { name: "More" }));
    const claude = (await screen.findByText("Claude Code")).closest("li")!;
    expect(within(claude).getByText("Ready")).toBeInTheDocument();
    const codex = screen.getByText("Codex").closest("li")!;
    expect(
      within(codex).getByText("Plenipo gives Codex no tasks: its update failed."),
    ).toBeInTheDocument();
    const grok = screen.getByText("Grok Build").closest("li")!;
    expect(
      within(grok).getByText("Can't work yet: sign in, or add its key, in AI tools on your PC."),
    ).toBeInTheDocument();
    expect(screen.getByText("Not on your PC: Kimi Code.")).toBeInTheDocument();
    expect(screen.queryByText("claude-code")).not.toBeInTheDocument();
  });
});

describe("part 14B: everything else that is safe from the page", () => {
  const position = {
    id: "p1",
    title: "Website Supervisor",
    roleName: "Supervisor",
    active: true,
    staffing: "persistent",
    agent: { runtimeId: "claude", sessionId: "s1" },
    currentTask: null,
    statusDetail: null,
  };
  const running = {
    id: "t5",
    objective: "Make the website faster",
    state: "running",
    positionId: "p1",
    positionTitle: "Website Supervisor",
    projectId: null,
    parentTaskId: null,
    sessionId: "s1",
    createdAt: Date.now() - 60_000,
    startedAt: Date.now() - 60_000,
    completedAt: null,
  };

  function withWork(extra: (ask: PhoneAsk) => unknown = () => undefined) {
    const answer = pc.answer;
    pc.answer = (ask) => {
      const mine = extra(ask);
      if (mine !== undefined) return mine;
      switch (ask.kind) {
        case "readOrganization":
          return { positions: [position], projects: [] };
        case "readWorkers":
          return { running: [running], waiting: [], queued: [], recent: [] };
        case "readTasks":
          return {
            session: { id: "s1", title: "Website Supervisor" },
            turns: [
              {
                taskId: "t6",
                objective: "Check the shop's pages",
                running: true,
                waiting: false,
                result: null,
              },
            ],
          };
        case "stopTask":
          return { session: { id: "s1" }, turns: [] };
        case "sendObjective":
          return { conversation: "s1", task: "t6" };
        default:
          return answer(ask);
      }
    };
  }

  it("Allow again after Stop all, from the phone", async () => {
    pc.signedIn.add("cGhvbmUtMQ");
    let stopped = false;
    const answer = pc.answer;
    pc.answer = (ask) => {
      const control = { stopped, sessions: [], revision: 1 };
      switch (ask.kind) {
        case "readControl":
          return { control, recovery: [] };
        case "stopAll":
          stopped = true;
          return { ...control, stopped: true };
        case "allowAgain":
          stopped = false;
          return { ...control, stopped: false };
        default:
          return answer(ask);
      }
    };
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await pairedWith(pc))} make={pc.make} />);
    await screen.findByText("Make the website faster");
    await user.click(screen.getByRole("button", { name: "Stop all" }));
    const ask = screen.getByRole("alertdialog", { name: "Stop all?" });
    await user.click(within(ask).getByRole("button", { name: "Stop all" }));
    expect(await screen.findByText(/work is stopped\./)).toBeInTheDocument();
    await user.click(await screen.findByRole("button", { name: "Allow again" }));
    expect(await screen.findByRole("button", { name: "Stop all" })).toBeInTheDocument();
    expect(pc.asked).toContainEqual({ kind: "allowAgain" });
    expect(screen.queryByText(/work is stopped\./)).toBeNull();
  });

  it("Run again and Leave stopped, after Plenipo stopped unexpectedly", async () => {
    pc.signedIn.add("cGhvbmUtMQ");
    let shown = true;
    const answer = pc.answer;
    pc.answer = (ask) => {
      switch (ask.kind) {
        case "readControl":
          return {
            control: { stopped: false, sessions: [], revision: 1 },
            recovery: shown
              ? [
                  {
                    org: "first",
                    name: "8 West Ventures",
                    recovery: {
                      id: "r1",
                      cause: "crash",
                      lastSeenAt: null,
                      foundAt: Date.now(),
                      previousVersion: null,
                      stoppedPrograms: 0,
                      stoppedTasks: [
                        {
                          taskId: "t9",
                          objective: "Update the price list",
                          who: "Senior Developer",
                          canRunAgain: true,
                          runAgainAs: null,
                        },
                      ],
                    },
                  },
                ]
              : [],
          };
        case "runAgain":
          return { id: "r1" };
        case "leaveStopped":
          shown = false;
          return null;
        default:
          return answer(ask);
      }
    };
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await pairedWith(pc))} make={pc.make} />);
    const notice = (
      await screen.findByRole("heading", { name: "Plenipo closed unexpectedly on your PC" })
    ).closest("section")!;
    expect(within(notice).getByText("Update the price list")).toBeInTheDocument();
    expect(notice).toHaveTextContent("Nothing runs again until you choose Run again.");
    await user.click(within(notice).getByRole("button", { name: "Run again" }));
    // Each request reaches the PC through the sealed line, a moment after the tap.
    await waitFor(() =>
      expect(pc.asked).toContainEqual({ kind: "runAgain", org: "first", task: "t9" }),
    );
    await user.click(await within(notice).findByRole("button", { name: "Leave stopped" }));
    await waitFor(() =>
      expect(pc.asked).toContainEqual({ kind: "leaveStopped", org: "first", notice: "r1" }),
    );
    await waitFor(() =>
      expect(
        screen.queryByRole("heading", { name: "Plenipo closed unexpectedly on your PC" }),
      ).toBeNull(),
    );
  });

  it("stops a worker's task, after asking", async () => {
    pc.signedIn.add("cGhvbmUtMQ");
    withWork();
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await pairedWith(pc))} make={pc.make} />);
    await user.click(await screen.findByRole("button", { name: "Work" }));
    await user.click(await screen.findByRole("button", { name: "Their work" }));
    const task = (await screen.findByText("Make the website faster")).closest("li")!;
    await user.click(within(task).getByRole("button", { name: "Stop the worker" }));
    expect(pc.asked.some((a) => a.kind === "stopTask")).toBe(false);
    const ask = within(task).getByRole("alertdialog", { name: "Stop the worker?" });
    await user.click(within(ask).getByRole("button", { name: "Stop the worker" }));
    expect(await within(task).findByText("Stopped.")).toBeInTheDocument();
    expect(pc.asked).toContainEqual({ kind: "stopTask", org: "first", conversation: "s1" });
  });

  it("gives an objective in words, then shows its conversation", async () => {
    pc.signedIn.add("cGhvbmUtMQ");
    withWork((ask) =>
      ask.kind === "readWorkers" ? { running: [], waiting: [], queued: [], recent: [] } : undefined,
    );
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await pairedWith(pc))} make={pc.make} />);
    await user.click(await screen.findByRole("button", { name: "Work" }));
    await user.click(await screen.findByRole("button", { name: "Their work" }));
    const box = await screen.findByLabelText("Objective for Website Supervisor");
    const give = screen.getByRole("button", { name: "Give objective" });
    expect(give).toBeDisabled();
    await user.type(box, "Check the shop's pages");
    await user.click(give);
    await waitFor(() =>
      expect(pc.asked).toContainEqual({
        kind: "sendObjective",
        org: "first",
        position: "p1",
        text: "Check the shop's pages",
      }),
    );
    // Its conversation opens, with the worker on it.
    expect(await screen.findByRole("button", { name: "Stop the worker" })).toBeInTheDocument();
    expect(screen.getByText("Check the shop's pages")).toBeInTheDocument();
  });

  it("keeps or discards a lesson as written", async () => {
    pc.signedIn.add("cGhvbmUtMQ");
    let waiting = [
      {
        id: "l1",
        worker: "Senior Developer",
        text: "Run the tests before a push.",
        heldReason: null,
      },
      { id: "l2", worker: "Code Reviewer", text: "Read the whole file.", heldReason: null },
    ];
    const answer = pc.answer;
    pc.answer = (ask) => {
      switch (ask.kind) {
        case "readLessons":
          return { enabled: true, autoRoles: [], offRoles: [], agents: {}, waiting, kept: [] };
        case "keepLesson":
        case "discardLesson":
          waiting = waiting.filter((l) => l.id !== ask.lesson);
          return {};
        default:
          return answer(ask);
      }
    };
    const user = userEvent.setup();
    render(<App keep={memoryKeep(await pairedWith(pc))} make={pc.make} />);
    await user.click(await screen.findByRole("button", { name: "More" }));
    const first = (await screen.findByText("Run the tests before a push.")).closest("li")!;
    await user.click(within(first).getByRole("button", { name: "Keep" }));
    await waitFor(() =>
      expect(pc.asked).toContainEqual({ kind: "keepLesson", org: "first", lesson: "l1" }),
    );
    const second = (await screen.findByText("Read the whole file.")).closest("li")!;
    await user.click(await within(second).findByRole("button", { name: "Discard" }));
    await waitFor(() =>
      expect(pc.asked).toContainEqual({ kind: "discardLesson", org: "first", lesson: "l2" }),
    );
    expect(await screen.findByText("No lessons are waiting for you.")).toBeInTheDocument();
  });
});
