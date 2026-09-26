import { Fragment, useState, type FormEvent } from "react";
import type { Capability, Level, PermissionSet, PermissionsSnapshot } from "@plenipo/types";

import { removePermissionSet, savePermissionSet } from "../../api/commands";
import { CAPABILITY_LABEL, LEVELS, LEVEL_LABEL, levelOf, setSummary } from "../../guard/format";
import { useRun } from "../../guard/useRun";
import { Refusal } from "../models/shared";

type Apply = (s: PermissionsSnapshot) => void;

/** Permission sets: what each allows, and an editor. */
export function PermissionSets({
  snapshot,
  onApply,
}: {
  snapshot: PermissionsSnapshot;
  onApply: Apply;
}) {
  const [editing, setEditing] = useState<string | null>(null);
  const { pending, error, run } = useRun(onApply);
  const sets = snapshot.settings.sets;
  return (
    <section aria-labelledby="sets-title">
      <div className="section-header">
        <h3 id="sets-title">Permission sets</h3>
        <button
          type="button"
          className="button button--small"
          aria-expanded={editing === ""}
          onClick={() => setEditing(editing === "" ? null : "")}
        >
          New permission set
        </button>
      </div>
      <p className="muted">
        A permission set says what a worker may do: allowed, ask me each time, or blocked. Give a
        role a set to grant it; give a project or department a set to limit everything done there.
      </p>
      {editing === "" && (
        <SetEditor snapshot={snapshot} onApply={onApply} onDone={() => setEditing(null)} />
      )}
      <table className="table">
        <thead>
          <tr>
            <th scope="col">Set</th>
            <th scope="col">What it allows</th>
            <th scope="col">
              <span className="visually-hidden">Actions</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {sets.map((s) => (
            <Fragment key={s.id}>
              <tr aria-current={editing === s.id}>
                <th scope="row">
                  {s.name}
                  <span className="table__sub">{s.description}</span>
                </th>
                <td>{setSummary(s)}</td>
                <td>
                  <button
                    type="button"
                    className="button button--small button--quiet"
                    aria-expanded={editing === s.id}
                    aria-label={`Change the ${s.name} set`}
                    onClick={() => setEditing(editing === s.id ? null : s.id)}
                  >
                    {editing === s.id ? "Close" : "Change"}
                  </button>{" "}
                  {!s.builtIn && (
                    <button
                      type="button"
                      className="button button--small button--danger"
                      disabled={pending}
                      aria-label={`Remove the ${s.name} set`}
                      onClick={() => void run(() => removePermissionSet(s.id))}
                    >
                      Remove
                    </button>
                  )}
                </td>
              </tr>
              {editing === s.id && (
                <tr>
                  <td colSpan={3}>
                    <SetEditor
                      snapshot={snapshot}
                      set={s}
                      onApply={onApply}
                      onDone={() => setEditing(null)}
                    />
                  </td>
                </tr>
              )}
            </Fragment>
          ))}
        </tbody>
      </table>
      <Refusal error={error} />
    </section>
  );
}

function SetEditor({
  snapshot,
  set,
  onApply,
  onDone,
}: {
  snapshot: PermissionsSnapshot;
  set?: PermissionSet;
  onApply: Apply;
  onDone: () => void;
}) {
  const [name, setName] = useState(set?.name ?? "");
  const [description, setDescription] = useState(set?.description ?? "");
  const [levels, setLevels] = useState<Partial<Record<Capability, Level>>>(set?.levels ?? {});
  const { pending, error, run } = useRun(onApply);
  const title = set ? `Permissions in the ${set.name} set` : "New permission set";
  const submit = (e: FormEvent) => {
    e.preventDefault();
    void run(() =>
      savePermissionSet({
        ...(set ? { id: set.id } : {}),
        name: name.trim(),
        description: description.trim(),
        levels,
      }),
    ).then((ok) => {
      if (ok) onDone();
    });
  };
  return (
    <form className="permissions__editor" aria-label={title} onSubmit={submit}>
      <label className="field">
        <span>Name</span>
        <input value={name} maxLength={60} required onChange={(e) => setName(e.target.value)} />
      </label>
      <label className="field">
        <span>Description</span>
        <input
          value={description}
          maxLength={500}
          onChange={(e) => setDescription(e.target.value)}
        />
      </label>
      <table className="table permissions__grid">
        <thead>
          <tr>
            <th scope="col">Permission</th>
            {LEVELS.map((l) => (
              <th scope="col" key={l}>
                {LEVEL_LABEL[l]}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {snapshot.settings.capabilities.map((c) => {
            const current = levelOf({ levels } as PermissionSet, c.id);
            return (
              <tr key={c.id}>
                <th scope="row">
                  {CAPABILITY_LABEL[c.id]}
                  <span className="table__sub">
                    {c.description}
                    {!c.tools && c.arrives
                      ? ` (Plenipo's tools for this arrive in ${c.arrives}.)`
                      : ""}
                  </span>
                </th>
                {LEVELS.map((l) => (
                  <td key={l}>
                    <input
                      type="radio"
                      name={`level-${c.id}`}
                      aria-label={`${CAPABILITY_LABEL[c.id]}: ${LEVEL_LABEL[l]}`}
                      checked={current === l}
                      onChange={() => setLevels({ ...levels, [c.id]: l })}
                    />
                  </td>
                ))}
              </tr>
            );
          })}
        </tbody>
      </table>
      <Refusal error={error} />
      <div className="actions">
        <button type="submit" className="button" disabled={pending || name.trim() === ""}>
          Save permission set
        </button>
        <button type="button" className="button button--quiet" onClick={onDone}>
          Cancel
        </button>
      </div>
    </form>
  );
}
