import { useState, type FormEvent } from "react";
import type { PermissionsSnapshot, SecretInfo } from "@plenipo/types";
import { Button, StatusPill } from "@plenipo/ui";

import { removeSecret, saveSecret } from "../../api/commands";
import { useRun } from "../../guard/useRun";
import { Refusal } from "../models/shared";
import { PILL_TONE } from "../tones";

type Apply = (s: PermissionsSnapshot) => void;

/**
 * Programs that run whatever script they are handed. A secret given to one of them reaches every
 * script a worker runs with it, so the owner is warned where the binding is listed and edited
 * (ADR-038, secrets reach only the programs they are for).
 */
const INTERPRETERS = [
  "node",
  "python",
  "python3",
  "bash",
  "sh",
  "zsh",
  "pwsh",
  "powershell",
  "cmd",
  "deno",
  "bun",
];

/** The program names typed into the form, in order, without blanks. */
const programList = (text: string) =>
  text
    .split(/[,\s]+/)
    .map((p) => p.trim())
    .filter((p) => p !== "");

/** One warning for each interpreter among `programs`. */
function interpreterWarnings(programs: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const p of programs.map((p) => p.trim().toLowerCase())) {
    if (INTERPRETERS.includes(p) && !seen.has(p)) {
      seen.add(p);
      out.push(`Every script run with ${p} would get this secret.`);
    }
  }
  return out;
}

function Warnings({ programs }: { programs: string[] }) {
  return (
    <>
      {interpreterWarnings(programs).map((w) => (
        <p key={w} className="hint" role="note">
          {w}
        </p>
      ))}
    </>
  );
}

/**
 * The Vault: secrets kept in the operating system's protected storage. Plenipo keeps only the
 * name and where a secret may be used; the value is typed once, stored there, and never shown
 * again — not to you, and never to a worker.
 */
export function SecretList({
  snapshot,
  onApply,
}: {
  snapshot: PermissionsSnapshot;
  onApply: Apply;
}) {
  const [editing, setEditing] = useState<SecretInfo | "new" | null>(null);
  const { pending, error, run } = useRun(onApply);
  const vault = snapshot.vault;
  return (
    <section aria-labelledby="secrets-title">
      <div className="section-header">
        <h3 id="secrets-title">Secrets</h3>
        <Button
          variant="primary"
          size="sm"
          disabled={!vault.available}
          onClick={() => setEditing("new")}
        >
          Add a secret
        </Button>
      </div>
      <p className="muted">
        Kept in {vault.label}. Plenipo stores only each secret&apos;s name and which programs get it
        (as an environment variable), and hides the value wherever it would appear. A worker never
        sees it: it can only run an allowed program that uses it.
      </p>
      {!vault.available && (
        <p className="form-error" role="alert">
          {vault.label} is not available on this computer
          {vault.detail ? `: ${vault.detail}` : "."}
        </p>
      )}
      {editing !== null && (
        <SecretForm
          secret={editing === "new" ? null : editing}
          onApply={onApply}
          onDone={() => setEditing(null)}
        />
      )}
      {snapshot.settings.secrets.length === 0 ? (
        <p className="empty">No secrets stored.</p>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th scope="col">Secret</th>
              <th scope="col">Given to</th>
              <th scope="col">Stored</th>
              <th scope="col">
                <span className="visually-hidden">Actions</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {snapshot.settings.secrets.map((s) => (
              <tr key={s.id}>
                <th scope="row">{s.name}</th>
                <td>
                  {s.envVar && s.programs.length > 0
                    ? `${s.programs.join(", ")} as ${s.envVar}`
                    : "No program (hidden in text only)"}
                  <Warnings programs={s.programs} />
                </td>
                <td>
                  {vault.stored.includes(s.id) ? (
                    <StatusPill status={PILL_TONE.ok} label="Yes" />
                  ) : (
                    <StatusPill status={PILL_TONE.warn} label="Missing" />
                  )}
                </td>
                <td>
                  <Button
                    variant="quiet"
                    size="sm"
                    aria-label={`Change ${s.name}`}
                    onClick={() => setEditing(s)}
                  >
                    Change
                  </Button>{" "}
                  <Button
                    variant="danger"
                    size="sm"
                    aria-label={`Remove ${s.name}`}
                    disabled={pending}
                    onClick={() => void run(() => removeSecret(s.id))}
                  >
                    Remove
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <Refusal error={error} />
    </section>
  );
}

function SecretForm({
  secret,
  onApply,
  onDone,
}: {
  secret: SecretInfo | null;
  onApply: Apply;
  onDone: () => void;
}) {
  const [name, setName] = useState(secret?.name ?? "");
  const [value, setValue] = useState("");
  const [envVar, setEnvVar] = useState(secret?.envVar ?? "");
  const [programs, setPrograms] = useState(secret?.programs.join(", ") ?? "");
  const { pending, error, run } = useRun(onApply);
  const submit = (e: FormEvent) => {
    e.preventDefault();
    void run(() =>
      saveSecret({
        ...(secret ? { id: secret.id } : {}),
        name: name.trim(),
        ...(envVar.trim() ? { envVar: envVar.trim() } : {}),
        programs: programList(programs),
        ...(value ? { value } : {}),
      }),
    ).then((ok) => {
      setValue("");
      if (ok) onDone();
    });
  };
  return (
    <form
      className="permissions__editor"
      aria-label={secret ? `Change ${secret.name}` : "New secret"}
      onSubmit={submit}
    >
      <label className="field">
        <span>Name</span>
        <input value={name} maxLength={60} required onChange={(e) => setName(e.target.value)} />
      </label>
      <label className="field">
        <span>{secret ? "New value (leave empty to keep the stored one)" : "Value"}</span>
        <input
          type="password"
          autoComplete="off"
          value={value}
          required={!secret}
          onChange={(e) => setValue(e.target.value)}
        />
      </label>
      <label className="field">
        <span>Give it to these programs (optional, for example gh)</span>
        <input value={programs} onChange={(e) => setPrograms(e.target.value)} />
      </label>
      <Warnings programs={programList(programs)} />
      <label className="field">
        <span>As this environment variable (for example GH_TOKEN)</span>
        <input value={envVar} maxLength={64} onChange={(e) => setEnvVar(e.target.value)} />
      </label>
      <Refusal error={error} />
      <div className="actions">
        <Button type="submit" variant="primary" disabled={pending || name.trim() === ""}>
          {secret ? "Save secret" : "Store secret"}
        </Button>
        <Button variant="quiet" onClick={onDone}>
          Cancel
        </Button>
      </div>
    </form>
  );
}
