import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { BrowserStatus } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../../api/commands";
import * as events from "../../api/events";
import { samplePermissions } from "../../test/permissionFixtures";
import { PermissionSettings } from "./PermissionSettings";

vi.mock("../../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getPermissions: vi.fn(),
    savePermissionSet: vi.fn(),
    removePermissionSet: vi.fn(),
    assignPermissions: vi.fn(),
    setCommandRules: vi.fn(),
    setBlockedFiles: vi.fn(),
    setSensitiveRule: vi.fn(),
    setGuardOptions: vi.fn(),
    saveSecret: vi.fn(),
    removeSecret: vi.fn(),
    setWebsiteRules: vi.fn(),
    getBrowserStatus: vi.fn(),
    openBrowser: vi.fn(),
    setBrowserChoice: vi.fn(),
  };
});
vi.mock("../../api/events", () => ({
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);

const edge: BrowserStatus = {
  name: "Microsoft Edge",
  running: false,
  profile: "C:/Plenipo/browser-profile",
  choice: "automatic",
  options: [
    { choice: "edge", name: "Microsoft Edge", installed: true },
    { choice: "chrome", name: "Google Chrome", installed: true },
  ],
  fixed: false,
};

beforeEach(() => {
  api.getPermissions.mockResolvedValue(samplePermissions());
  for (const f of [
    api.savePermissionSet,
    api.removePermissionSet,
    api.assignPermissions,
    api.setCommandRules,
    api.setBlockedFiles,
    api.setSensitiveRule,
    api.setGuardOptions,
    api.saveSecret,
    api.removeSecret,
    api.setWebsiteRules,
  ]) {
    f.mockResolvedValue(samplePermissions());
  }
  api.getBrowserStatus.mockResolvedValue(edge);
  api.openBrowser.mockResolvedValue({ ...edge, running: true });
  void events;
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Settings → Permissions", () => {
  it("shows who may do what and changes a role's set", async () => {
    render(<PermissionSettings />);
    const dev = await screen.findByRole("combobox", { name: "Senior Developer's permission set" });
    expect(dev).toHaveValue("developer");
    const rev = screen.getByRole("combobox", { name: "Code Reviewer's permission set" });
    expect(rev).toHaveValue("");
    const user = userEvent.setup();
    await user.selectOptions(rev, "read-only");
    expect(api.assignPermissions).toHaveBeenCalledWith("role", "role-rev", "read-only");
    await user.selectOptions(dev, "");
    expect(api.assignPermissions).toHaveBeenCalledWith("role", "role-dev", null);
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Development's limit" }),
      "read-only",
    );
    expect(api.assignPermissions).toHaveBeenCalledWith("department", "dept-1", "read-only");
    // Projects show their folder and limit, and what to fix.
    const website = screen.getByRole("row", { name: /^Website/ });
    expect(within(website).getByText("D:\\projects\\website")).toBeInTheDocument();
    expect(within(website).getByText("Read only")).toBeInTheDocument();
    expect(screen.getByText(/It has no folder/)).toBeInTheDocument();
  });

  it("describes and edits a permission set", async () => {
    render(<PermissionSettings />);
    const row = await screen.findByRole("row", { name: /^Developer/ });
    expect(
      within(row).getByText(
        "Read files, Change files, Run programs, Read git history, Save to git · asks you before: Run PowerShell scripts",
      ),
    ).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(within(row).getByRole("button", { name: "Change the Developer set" }));
    const form = screen.getByRole("form", { name: "Permissions in the Developer set" });
    // Tools arriving later say so.
    expect(within(form).getAllByText(/arrive in Phase 10/).length).toBeGreaterThan(0);
    await user.click(within(form).getByRole("radio", { name: "Change files: Ask me" }));
    await user.click(within(form).getByRole("radio", { name: "Run programs: Blocked" }));
    await user.click(within(form).getByRole("button", { name: "Save permission set" }));
    expect(api.savePermissionSet).toHaveBeenCalledWith({
      id: "developer",
      name: "Developer",
      description: "Developer work.",
      levels: {
        "filesystem.read": "allowed",
        "filesystem.write": "ask",
        "shell.exec": "blocked",
        "powershell.exec": "ask",
        "git.read": "allowed",
        "git.write": "allowed",
      },
    });
    // Built-in sets stay; one of yours can be removed.
    expect(within(row).queryByRole("button", { name: /Remove/ })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Remove the Docs set" }));
    expect(api.removePermissionSet).toHaveBeenCalledWith("docs");
  });

  it("adds a new permission set", async () => {
    render(<PermissionSettings />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "New permission set" }));
    const form = screen.getByRole("form", { name: "New permission set" });
    await user.type(within(form).getByLabelText("Name"), "Testers");
    await user.click(within(form).getByRole("radio", { name: "Read files: Allowed" }));
    await user.click(within(form).getByRole("button", { name: "Save permission set" }));
    expect(api.savePermissionSet).toHaveBeenCalledWith({
      name: "Testers",
      description: "",
      levels: { "filesystem.read": "allowed" },
    });
  });

  it("says that approved programs run with the owner's account", async () => {
    render(<PermissionSettings />);
    await screen.findByRole("form", { name: "Command lists" });
    // The note holds <code>npm run</code>, so match the whole paragraph's text.
    const note = screen.getByText(
      (_, el) =>
        el?.tagName === "P" &&
        /Approved programs run with your full account/.test(el.textContent ?? ""),
    );
    expect(note).toHaveTextContent(
      "Approved programs run with your full account, and a project's own build and test scripts run as part of them. Approve script runners like npm run only for projects you trust.",
    );
  });

  it("saves the command and file lists, one per line", async () => {
    render(<PermissionSettings />);
    const commandsForm = await screen.findByRole("form", { name: "Command lists" });
    const user = userEvent.setup();
    const approved = within(commandsForm).getByLabelText("Approved: run without asking");
    expect(approved).toHaveValue("cargo test *\nnpm test *");
    await user.type(approved, "\n\n  pnpm test *  ");
    await user.type(within(commandsForm).getByLabelText("Always ask me first"), "npm publish *");
    await user.click(within(commandsForm).getByRole("button", { name: "Save command lists" }));
    expect(api.setCommandRules).toHaveBeenCalledWith({
      approved: ["cargo test *", "npm test *", "pnpm test *"],
      ask: ["npm publish *"],
      blocked: ["curl *"],
      withSecrets: [],
    });
    const files = screen.getByRole("form", { name: "Blocked files" });
    await user.type(within(files).getByLabelText("Blocked files"), "\nsecrets/");
    await user.click(within(files).getByRole("button", { name: "Save blocked files" }));
    expect(api.setBlockedFiles).toHaveBeenCalledWith([".env", "*.pem", "secrets/"]);
  });

  it("manages the website lists and opens Plenipo's browser for the owner to sign in", async () => {
    render(<PermissionSettings />);
    const form = await screen.findByRole("form", { name: "Website lists" });
    expect(screen.getByRole("note")).toHaveTextContent(
      "Check a website's terms before you allow it.",
    );
    const user = userEvent.setup();
    const allowed = within(form).getByLabelText("Allowed (open without asking)");
    expect(allowed).toHaveValue("example.com");
    await user.type(allowed, "\n  shop.example.org ");
    await user.type(within(form).getByLabelText("Blocked (never open)"), "\nfacebook.com");
    await user.selectOptions(within(form).getByLabelText("Other websites"), "block");
    await user.click(within(form).getByRole("button", { name: "Save websites" }));
    expect(api.setWebsiteRules).toHaveBeenCalledWith({
      allowed: ["example.com", "shop.example.org"],
      blocked: ["linkedin.com", "facebook.com"],
      others: "block",
    });
    const browser = screen.getByLabelText("Plenipo's browser");
    expect(
      await within(browser).findByText(/Microsoft Edge, with its own profile/),
    ).toBeInTheDocument();
    expect(within(browser).getByText("Not open")).toBeInTheDocument();
    await user.type(within(browser).getByLabelText("Website to open"), "https://example.com/login");
    await user.click(within(browser).getByRole("button", { name: "Open Plenipo's browser" }));
    expect(api.openBrowser).toHaveBeenCalledWith("https://example.com/login");
    expect(await within(browser).findByText("Open")).toBeInTheDocument();
  });

  it("chooses which browser is Plenipo's browser", async () => {
    api.getBrowserStatus.mockResolvedValue({ ...edge, running: true });
    api.setBrowserChoice.mockResolvedValue({
      ...edge,
      running: true,
      choice: "chrome",
      next: "Google Chrome",
    });
    render(<PermissionSettings />);
    const browser = await screen.findByLabelText("Plenipo's browser");
    const menu = await within(browser).findByLabelText("Browser");
    expect(menu).toHaveValue("automatic");
    await userEvent.setup().selectOptions(menu, "chrome");
    expect(api.setBrowserChoice).toHaveBeenCalledWith("chrome");
    expect(
      await within(browser).findByText(/switches to Google Chrome the next time/),
    ).toBeInTheDocument();
    expect(within(browser).getByText(/Each browser keeps its own sign-ins/)).toBeInTheDocument();
  });

  it("offers only the browsers on this computer, and none when PLENIPO_BROWSER names one", async () => {
    api.getBrowserStatus.mockResolvedValue({
      ...edge,
      options: [
        { choice: "edge", name: "Microsoft Edge", installed: true },
        { choice: "chrome", name: "Google Chrome", installed: false },
      ],
    });
    render(<PermissionSettings />);
    const browser = await screen.findByLabelText("Plenipo's browser");
    const menu = await within(browser).findByLabelText("Browser");
    const chrome = within(menu).getByRole("option", { name: "Google Chrome (not installed)" });
    expect(chrome).toBeDisabled();
    cleanup();
    api.getBrowserStatus.mockResolvedValue({ ...edge, fixed: true });
    render(<PermissionSettings />);
    const fixed = await screen.findByLabelText("Plenipo's browser");
    expect(await within(fixed).findByLabelText("Browser")).toBeDisabled();
    expect(within(fixed).getByText(/PLENIPO_BROWSER setting/)).toBeInTheDocument();
  });

  it("sets what a sensitive action does and how long approvals wait", async () => {
    render(<PermissionSettings />);
    const user = userEvent.setup();
    const outbound = await screen.findByRole("combobox", {
      name: "Sending or publishing outside this computer: what happens",
    });
    expect(outbound).toHaveValue("ask");
    // Never "allowed without asking".
    expect(
      within(outbound)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual(["Ask me", "Blocked"]);
    await user.selectOptions(outbound, "block");
    expect(api.setSensitiveRule).toHaveBeenCalledWith("outbound", "block");
    const wait = screen.getByRole("form", { name: "Approval wait" });
    const minutes = within(wait).getByRole("spinbutton");
    await user.clear(minutes);
    await user.type(minutes, "15");
    await user.click(within(wait).getByRole("button", { name: "Save" }));
    expect(api.setGuardOptions).toHaveBeenCalledWith({ approvalMinutes: 15 });
  });

  it("stores a secret without ever showing its value", async () => {
    render(<PermissionSettings />);
    const row = await screen.findByRole("row", { name: /^GitHub token/ });
    expect(within(row).getByText("gh as GH_TOKEN")).toBeInTheDocument();
    expect(within(row).getByText("Yes")).toBeInTheDocument();
    expect(screen.getByText(/Kept in Windows Credential Manager/)).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Add a secret" }));
    const form = screen.getByRole("form", { name: "New secret" });
    await user.type(within(form).getByLabelText("Name"), "npm token");
    const value = within(form).getByLabelText("Value");
    expect(value).toHaveAttribute("type", "password");
    await user.type(value, "npm_secret_value");
    await user.type(within(form).getByLabelText(/Give it to these programs/), "npm, pnpm");
    await user.type(within(form).getByLabelText(/As this environment variable/), "NPM_TOKEN");
    await user.click(within(form).getByRole("button", { name: "Store secret" }));
    expect(api.saveSecret).toHaveBeenCalledWith({
      name: "npm token",
      envVar: "NPM_TOKEN",
      programs: ["npm", "pnpm"],
      value: "npm_secret_value",
    });
    expect(screen.queryByDisplayValue("npm_secret_value")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Remove GitHub token" }));
    expect(api.removeSecret).toHaveBeenCalledWith("secret-1");
  });

  it("warns where a secret is bound to a program that runs scripts", async () => {
    // ADR-048 (secrets reach only the programs they are for): the warning shows in the list...
    const base = samplePermissions();
    api.getPermissions.mockResolvedValue({
      ...base,
      settings: {
        ...base.settings,
        secrets: [
          ...base.settings.secrets,
          {
            id: "secret-2",
            name: "PyPI token",
            envVar: "TWINE_PASSWORD",
            programs: ["python", "twine"],
            createdAt: 0,
            updatedAt: 0,
          },
        ],
      },
    });
    render(<PermissionSettings />);
    const row = await screen.findByRole("row", { name: /^PyPI token/ });
    expect(within(row).getByRole("note")).toHaveTextContent(
      "Every script run with python would get this secret.",
    );
    expect(
      within(screen.getByRole("row", { name: /^GitHub token/ })).queryByRole("note"),
    ).toBeNull();
    // ...and in the form, as the programs are typed.
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Add a secret" }));
    const form = screen.getByRole("form", { name: "New secret" });
    const programs = within(form).getByLabelText(/Give it to these programs/);
    await user.type(programs, "gh, node python3");
    expect(
      within(form)
        .getAllByRole("note")
        .map((n) => n.textContent),
    ).toEqual([
      "Every script run with node would get this secret.",
      "Every script run with python3 would get this secret.",
    ]);
    await user.clear(programs);
    await user.type(programs, "gh");
    expect(within(form).queryByRole("note")).toBeNull();
  });

  it("explains when the operating system's store or the tools are unavailable", async () => {
    api.getPermissions.mockResolvedValue(
      samplePermissions({
        vault: {
          available: false,
          label: "the Linux kernel keyring",
          detail: "Operation not permitted",
          stored: [],
        },
        tools: { running: false, detail: "Not running" },
        notices: ["Plenipo's tool server is not running, so workers get no tools."],
      }),
    );
    render(<PermissionSettings />);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "the Linux kernel keyring is not available on this computer: Operation not permitted",
    );
    expect(screen.getByRole("button", { name: "Add a secret" })).toBeDisabled();
    expect(screen.getByText("Tools not running")).toBeInTheDocument();
    expect(screen.getByText(/workers get no tools/)).toBeInTheDocument();
    expect(screen.getByText("Missing")).toBeInTheDocument();
  });
});
