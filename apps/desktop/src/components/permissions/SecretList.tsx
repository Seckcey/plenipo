import { useState, type FormEvent } from "react";
import type { PermissionsSnapshot, SecretInfo } from "@plenipo/types";

import { removeSecret, saveSecret } from "../../api/commands";
import { useRun } from "../../guard/useRun";
import { Refusal } from "../models/shared";

type Apply = (s: PermissionsSnapshot) => void;

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
        <button
          type="button"
          className="button button--small"
          disabled={!vault.available}
          onClick={() => setEditing("new")}
        >
          Add a secret
        </button>
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
                </td>
                <td>
                  {vault.stored.includes(s.id) ? (
                    <span className="pill pill--ok">Yes</span>
                  ) : (
                    <span className="pill pill--warn">Missing</span>
                  )}
                </td>
                <td>
                  <button
                    type="button"
                    className="button button--small button--quiet"
                    aria-label={`Change ${s.name}`}
                    onClick={() => setEditing(s)}
                  >
                    Change
                  </button>{" "}
                  <button
                    type="button"
                    className="button button--small button--danger"
                    aria-label={`Remove ${s.name}`}
                    disabled={pending}
                    onClick={() => void run(() => removeSecret(s.id))}
                  >
                    Remove
                  </button>
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
    const list = programs
      .split(/[,\s]+/)
      .map((p) => p.trim())
      .filter((p) => p !== "");
    void run(() =>
      saveSecret({
        ...(secret ? { id: secret.id } : {}),
        name: name.trim(),
        ...(envVar.trim() ? { envVar: envVar.trim() } : {}),
        programs: list,
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
      <label className="field">
        <span>As this environment variable (for example GH_TOKEN)</span>
        <input value={envVar} maxLength={64} onChange={(e) => setEnvVar(e.target.value)} />
      </label>
      <Refusal error={error} />
      <div className="actions">
        <button type="submit" className="button" disabled={pending || name.trim() === ""}>
          {secret ? "Save secret" : "Store secret"}
        </button>
        <button type="button" className="button button--quiet" onClick={onDone}>
          Cancel
        </button>
      </div>
    </form>
  );
}
