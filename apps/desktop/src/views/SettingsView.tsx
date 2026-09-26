import { useAgents } from "../agents/useAgents";
import { ModelSettings } from "../components/models/ModelSettings";
import { PermissionSettings } from "../components/permissions/PermissionSettings";
import { TitlesSetting } from "../components/TitlesSetting";
import { useRuntime } from "../runtime/useRuntime";

export function SettingsView() {
  const { state } = useRuntime();
  const agents = useAgents();
  return (
    <section className="view" aria-labelledby="settings-title">
      <h1 id="settings-title">Settings</h1>
      <p className="view__lead">
        Personalization, the AI models your roles use, and what workers may do on this computer can
        be changed here. The rest is shown for reference and becomes editable in later phases.
      </p>

      <h2>Personalization</h2>
      <TitlesSetting />

      <h2>AI models</h2>
      <ModelSettings />

      <h2>Permissions</h2>
      <PermissionSettings />

      <h2>AI tools</h2>
      <ul className="settings">
        <li>
          <strong>AI tools:</strong>{" "}
          {agents.state.runtimes.map((r) => r.label).join(", ") || "none found yet"}. Plenipo uses
          each tool&apos;s own sign-in on this computer and never asks for passwords.
        </li>
        <li>
          <strong>Billing:</strong> subscription sign-ins only. A tool signed in with an API key or
          through a third-party cloud is refused, and Plenipo never falls back to pay-per-use API
          billing. Reaching a usage limit never moves work to another AI company.
        </li>
        <li>
          <strong>Permissions:</strong> workers never get their AI tool&apos;s own tools or your
          add-ons (MCP servers); Codex&apos;s own commands stay read-only, without internet access.
          Workers of your organization with permissions (above) get Plenipo&apos;s own tools
          instead, confined to their project&apos;s folder and checked by Plenipo Guard. Tasks you
          start yourself in Workers get no tools.
        </li>
        <li>
          <strong>Your keys and secrets:</strong> API keys and other secrets on this computer are
          never passed to a worker. Only proxy settings and the tools&apos; own settings folders
          are. Secrets you store under Permissions go only to the programs you name, and are hidden
          wherever they would appear.
        </li>
        <li>
          <strong>Model:</strong> organization workers get the model their role&apos;s choices pick
          (above), unless you fixed an AI tool on the position. Tasks you start yourself in Workers
          use the AI tool&apos;s default unless you name a model.
        </li>
        <li>
          <strong>Handoffs:</strong> off unless you allow them for a new task. A worker may then ask
          a worker on another AI tool for help through Plenipo Liaison — workers never contact each
          other directly. Liaison records a sub-task, passes on only the context the worker chose
          (up to a limit), and brings the reply back to the same piece of work. Limits: 3 levels
          deep, 3 requests per answer, 5 reply rounds per task, 12 handoffs per piece of work. A
          worker&apos;s permissions come from your settings for its role and project; a request for
          more permissions is recorded but never grants anything.
        </li>
      </ul>

      <h2>Programs Plenipo runs</h2>
      <ul className="settings">
        <li>
          <strong>Approved programs:</strong>{" "}
          {state.profiles.map((p) => p.label).join(", ") || "none"}
        </li>
        <li>
          <strong>Only these:</strong> Plenipo itself (built-in checks), the AI tools it finds
          (Claude Code, Codex, Grok), and programs a worker runs with your permission (Permissions
          above). Nothing on screen can supply a command, path, or argument.
        </li>
        <li>
          <strong>Environment:</strong> programs get the operating system&apos;s basics plus
          settings Plenipo sets on purpose. Your credentials are never passed on.
        </li>
        <li>
          <strong>Kept apart:</strong> each run is its own group of processes; cancelling or
          quitting stops the whole group.
        </li>
      </ul>

      <h2>Window behavior</h2>
      <ul className="settings">
        <li>
          Closing the window while programs are running keeps them running; use the tray icon to
          reopen Plenipo or stop them.
        </li>
        <li>Quitting from the tray stops everything that is running and records how it ended.</li>
      </ul>
    </section>
  );
}
