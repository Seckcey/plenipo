import { useAgents } from "../agents/useAgents";
import { useRuntime } from "../runtime/useRuntime";

export function SettingsView() {
  const { state } = useRuntime();
  const agents = useAgents();
  return (
    <section className="view" aria-labelledby="settings-title">
      <h1 id="settings-title">Settings</h1>
      <p className="view__lead">
        Read-only in this build. Editable settings arrive in later phases.
      </p>

      <h2>Agent runtimes</h2>
      <ul className="settings">
        <li>
          <strong>Runtimes:</strong>{" "}
          {agents.state.runtimes.map((r) => r.label).join(", ") || "none detected yet"}. Plenipo
          uses each tool&apos;s own sign-in on this computer and never asks for passwords.
        </li>
        <li>
          <strong>Billing:</strong> subscription sign-ins only. A runtime signed in with an API key
          or a third-party cloud is refused, and API billing fallback is disabled. A usage limit
          never switches work to another provider.
        </li>
        <li>
          <strong>Permissions:</strong> Claude Code workers get no tools and no MCP servers; Codex
          workers run in their read-only sandbox without network access. Each session works in its
          own empty folder. Capability grants arrive with Plenipo Guard.
        </li>
        <li>
          <strong>Credentials:</strong> API keys and other secrets in Plenipo&apos;s environment are
          never passed to a worker. Only proxy settings and the tools&apos; own config locations
          are.
        </li>
        <li>
          <strong>Model:</strong> the runtime&apos;s default unless you name one for a new task.
          Automatic model selection arrives with role policies.
        </li>
        <li>
          <strong>Handoffs:</strong> off unless you allow them for a new task. A worker may then ask
          a worker on another runtime for help through Plenipo Liaison — workers never contact each
          other directly. Liaison creates a recorded sub-task, passes only the context the worker
          chose (capped), and returns the reply to the same workflow. Limits: depth 3, 3 requests
          per answer, 5 reply rounds per task, 12 handoffs per workflow. Handoff workers get the
          same permissions as any worker; capability requests are recorded but never granted before
          Plenipo Guard.
        </li>
      </ul>

      <h2>Runtime policy</h2>
      <ul className="settings">
        <li>
          <strong>Approved launch profiles:</strong>{" "}
          {state.profiles.map((p) => p.label).join(", ") || "none"}
        </li>
        <li>
          <strong>Executables:</strong> Plenipo itself (built-in diagnostics) and the detected
          Claude Code and Codex tools. The interface can never supply a command, path, or argument.
        </li>
        <li>
          <strong>Environment:</strong> child processes receive a minimal operating-system baseline
          plus variables declared by Plenipo. Your credentials are never passed on.
        </li>
        <li>
          <strong>Isolation:</strong> each launch runs in its own process tree; cancelling or
          quitting terminates the whole tree.
        </li>
      </ul>

      <h2>Window behavior</h2>
      <ul className="settings">
        <li>
          Closing the window while processes are running keeps them running; use the tray icon to
          reopen Plenipo or stop them.
        </li>
        <li>Quitting from the tray stops all running processes and records their final state.</li>
      </ul>
    </section>
  );
}
