import { useRuntime } from "../runtime/useRuntime";

export function SettingsView() {
  const { state } = useRuntime();
  return (
    <section className="view" aria-labelledby="settings-title">
      <h1 id="settings-title">Settings</h1>
      <p className="view__lead">
        Read-only in this build. Editable settings arrive in later phases.
      </p>

      <h2>Runtime policy</h2>
      <ul className="settings">
        <li>
          <strong>Approved launch profiles:</strong>{" "}
          {state.profiles.map((p) => p.label).join(", ") || "none"}
        </li>
        <li>
          <strong>Executables:</strong> only Plenipo itself, running built-in diagnostic scenarios.
          The interface can never supply a command, path, or argument.
        </li>
        <li>
          <strong>Environment:</strong> child processes receive a minimal operating-system baseline
          plus variables declared by the profile. Your credentials are never passed on.
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
