import { useState, type FormEvent } from "react";
import type { CommandRules, PermissionsSnapshot, SensitiveRule } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import {
  setBlockedFiles,
  setCommandRules,
  setGuardOptions,
  setSensitiveRule,
} from "../../api/commands";
import { SENSITIVE_RULE_LABEL } from "../../guard/format";
import { useRun } from "../../guard/useRun";
import { Refusal } from "../models/shared";

type Apply = (s: PermissionsSnapshot) => void;

const lines = (text: string) =>
  text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l !== "");

/** Commands: approved (run without asking), always ask, and never run. */
export function CommandLists({
  snapshot,
  onApply,
}: {
  snapshot: PermissionsSnapshot;
  onApply: Apply;
}) {
  const c = snapshot.settings.commands;
  const [approved, setApproved] = useState(c.approved.join("\n"));
  const [ask, setAsk] = useState(c.ask.join("\n"));
  const [blocked, setBlocked] = useState(c.blocked.join("\n"));
  const { pending, error, run } = useRun(onApply);
  const submit = (e: FormEvent) => {
    e.preventDefault();
    const rules: CommandRules = {
      approved: lines(approved),
      ask: lines(ask),
      blocked: lines(blocked),
      // Rules that name a program and its secrets (ADR-038) are kept as they are.
      withSecrets: c.withSecrets,
    };
    void run(() => setCommandRules(rules));
  };
  return (
    <section aria-labelledby="commands-title">
      <h3 id="commands-title">Programs workers may run</h3>
      <p className="muted">
        One command per line: a program name, then its arguments; <code>*</code> at the end matches
        any further arguments (<code>cargo test *</code>). Workers name a program and its arguments
        — never a shell command line. When a worker may run programs, approved commands run at once
        and anything else asks you. Sensitive actions (below) always ask.
      </p>
      <p className="muted">
        Approved programs run with your full account, and a project&apos;s own build and test
        scripts run as part of them. Approve script runners like <code>npm run</code> only for
        projects you trust.
      </p>
      <p className="muted">
        A program that would be given one of your stored secrets asks you first, even when it is on
        the approved list, and the approval card says which secrets. Only the installed program gets
        a secret, never a file inside a project folder with the same name. A rule that names both
        the program and the secret can let it run without asking; this screen cannot add one yet.
      </p>
      <form className="permissions__rules" aria-label="Command lists" onSubmit={submit}>
        <label className="field">
          <span>Approved: run without asking</span>
          <textarea rows={8} value={approved} onChange={(e) => setApproved(e.target.value)} />
        </label>
        <label className="field">
          <span>Always ask me first</span>
          <textarea rows={8} value={ask} onChange={(e) => setAsk(e.target.value)} />
        </label>
        <label className="field">
          <span>Never run</span>
          <textarea rows={8} value={blocked} onChange={(e) => setBlocked(e.target.value)} />
        </label>
        <div className="actions">
          <Button type="submit" variant="primary" disabled={pending}>
            Save command lists
          </Button>
        </div>
        <Refusal error={error} />
      </form>
    </section>
  );
}

/** Files no worker may open or change. */
export function BlockedFiles({
  snapshot,
  onApply,
}: {
  snapshot: PermissionsSnapshot;
  onApply: Apply;
}) {
  const [text, setText] = useState(snapshot.settings.blockedFiles.join("\n"));
  const { pending, error, run } = useRun(onApply);
  return (
    <section aria-labelledby="files-title">
      <h3 id="files-title">Files workers may never open</h3>
      <p className="muted">
        One pattern per line, like a .gitignore file: <code>*.pem</code> matches in any folder,
        <code> config/secrets.json</code> from the project folder&apos;s top, and{" "}
        <code>!.env.example</code> makes an exception.
      </p>
      <form
        aria-label="Blocked files"
        onSubmit={(e) => {
          e.preventDefault();
          void run(() => setBlockedFiles(lines(text)));
        }}
      >
        <label className="field">
          <span>Blocked files</span>
          <textarea rows={6} value={text} onChange={(e) => setText(e.target.value)} />
        </label>
        <div className="actions">
          <Button type="submit" variant="primary" disabled={pending}>
            Save blocked files
          </Button>
        </div>
        <Refusal error={error} />
      </form>
    </section>
  );
}

/** Sensitive actions: always ask (default) or never. */
export function SensitiveActions({
  snapshot,
  onApply,
}: {
  snapshot: PermissionsSnapshot;
  onApply: Apply;
}) {
  const { pending, error, run } = useRun(onApply);
  return (
    <section aria-labelledby="sensitive-title">
      <h3 id="sensitive-title">Sensitive actions</h3>
      <p className="muted">
        These always stop for your approval, even when a worker&apos;s permissions allow the program
        — or never happen at all, if you block them. Plenipo recognizes them in commands and
        scripts; the approved list above stays the main safeguard.
      </p>
      <table className="table">
        <thead>
          <tr>
            <th scope="col">Action</th>
            <th scope="col">Examples</th>
            <th scope="col">What happens</th>
          </tr>
        </thead>
        <tbody>
          {snapshot.settings.sensitive.map((k) => (
            <tr key={k.kind}>
              <th scope="row">{k.label}</th>
              <td className="table__sub">{k.examples}</td>
              <td>
                <select
                  aria-label={`${k.label}: what happens`}
                  value={k.rule}
                  disabled={pending}
                  onChange={(e) =>
                    void run(() => setSensitiveRule(k.kind, e.target.value as SensitiveRule))
                  }
                >
                  {(Object.keys(SENSITIVE_RULE_LABEL) as SensitiveRule[]).map((r) => (
                    <option key={r} value={r}>
                      {SENSITIVE_RULE_LABEL[r]}
                    </option>
                  ))}
                </select>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <Refusal error={error} />
    </section>
  );
}

/** How long a request waits for your answer. */
export function ApprovalWindow({
  snapshot,
  onApply,
}: {
  snapshot: PermissionsSnapshot;
  onApply: Apply;
}) {
  const [minutes, setMinutes] = useState(String(snapshot.settings.options.approvalMinutes));
  const { pending, error, run } = useRun(onApply);
  return (
    <section aria-labelledby="window-title">
      <h3 id="window-title">How long workers wait for your answer</h3>
      <form
        className="field--inline"
        aria-label="Approval wait"
        onSubmit={(e) => {
          e.preventDefault();
          void run(() => setGuardOptions({ approvalMinutes: Number(minutes) }));
        }}
      >
        <label className="field">
          <span>Minutes (1–60). With no answer by then, the request is not approved.</span>
          <input
            type="number"
            min={1}
            max={60}
            value={minutes}
            onChange={(e) => setMinutes(e.target.value)}
          />
        </label>
        <Button type="submit" variant="primary" size="sm" disabled={pending}>
          Save
        </Button>
      </form>
      <Refusal error={error} />
    </section>
  );
}
