import { useState } from "react";
import type { AddOn, AddOnTool, ConnectionsPage, ToolMark } from "@plenipo/types";
import { Button, Disclosure, Segmented, StatusPill, TextField } from "@plenipo/ui";

import {
  addAddOn,
  changeAddOn,
  checkAddOnTools,
  removeAddOn,
  setAddOnTools,
} from "../../api/commands";
import { Refusal } from "../../components/models/shared";
import { useRun } from "../../guard/useRun";
import { WhoMayUse } from "./WhoMayUse";
import { whoMayUseWords } from "./words";

const MARKS: readonly ToolMark[] = ["off", "reading", "changing"];
const MARK_LABEL: Record<ToolMark, string> = {
  off: "Off",
  reading: "Reading",
  changing: "Changing",
};

/**
 * **Add-on tools** (ADR-066, ADR-071): other programs that offer tools, which the owner installed
 * and adds here. Each starts **off**; switching it on looks at its tools, and each tool starts
 * **Off** until the owner marks it **Reading** (goes ahead) or **Changing** (asks every time).
 * Nobody may use one until the owner picks. Plenipo suggests no program.
 */
export function AddOnTools({
  page,
  onApply,
}: {
  page: ConnectionsPage;
  onApply: (page: ConnectionsPage) => void;
}) {
  return (
    <section className="connections__add-ons" aria-labelledby="add-on-tools">
      <h3 id="add-on-tools">Add-on tools</h3>
      <p className="muted">
        Add a program you installed that offers tools (programs that speak MCP, the Model Context
        Protocol) — for example a service&apos;s own tool program. Plenipo runs it for a
        worker&apos;s step and stops it after, gives it only the stored secrets you name, and shows
        its answers to workers as the program&apos;s words, never your instructions.
      </p>
      <p className="notice-box" role="note">
        <strong>This program runs with your full account, like any approved program.</strong> Add
        only programs from a publisher you trust — ideally the service&apos;s own. Programs that
        download code each time they start (npx, uvx, bunx, and the like) are refused: install the
        program first, then add the installed copy.
      </p>
      {page.addOns.length === 0 ? (
        <p className="empty">No add-on programs yet.</p>
      ) : (
        <ul className="connection-list" aria-label="Add-on programs">
          {page.addOns.map((a) => (
            <AddOnCard key={a.id} addOn={a} page={page} onApply={onApply} />
          ))}
        </ul>
      )}
      <AddProgram secretNames={page.secretNames} onApply={onApply} />
    </section>
  );
}

function AddProgram({
  secretNames,
  onApply,
}: {
  secretNames: string[];
  onApply: (page: ConnectionsPage) => void;
}) {
  const [name, setName] = useState("");
  const [program, setProgram] = useState("");
  const [args, setArgs] = useState("");
  const [secrets, setSecrets] = useState<string[]>([]);
  const { pending, error, run } = useRun(onApply);
  const argList = args
    .split("\n")
    .map((a) => a.trim())
    .filter((a) => a !== "");
  return (
    <form
      className="connection connection--add"
      aria-label="Add a program"
      onSubmit={(ev) => {
        ev.preventDefault();
        void run(() =>
          addAddOn({ name: name.trim(), program: program.trim(), args: argList, secrets }),
        ).then((ok) => {
          if (ok) {
            setName("");
            setProgram("");
            setArgs("");
            setSecrets([]);
          }
        });
      }}
    >
      <h4 id="add-a-program">Add a program</h4>
      <TextField label="Name" value={name} placeholder="Notion" onChange={setName} />
      <TextField
        label="Program"
        value={program}
        placeholder="notion-mcp-server, or its full path"
        hint="A program Plenipo can find by its name, or its full path. Never a shell."
        onChange={setProgram}
      />
      <label className="field">
        <span>Arguments, one a line</span>
        <textarea rows={3} value={args} onChange={(e) => setArgs(e.target.value)} />
      </label>
      {secretNames.length > 0 && (
        <fieldset className="field">
          <legend>Stored secrets to give it (Settings → Secrets)</legend>
          {secretNames.map((s) => (
            <label key={s} className="check">
              <input
                type="checkbox"
                checked={secrets.includes(s)}
                onChange={(e) =>
                  setSecrets(e.target.checked ? [...secrets, s] : secrets.filter((x) => x !== s))
                }
              />{" "}
              {s}
            </label>
          ))}
        </fieldset>
      )}
      <div className="actions">
        <Button
          type="submit"
          variant="secondary"
          size="sm"
          disabled={pending || name.trim() === "" || program.trim() === ""}
        >
          Add a program
        </Button>
      </div>
      <Refusal error={error} />
    </form>
  );
}

/** The program's own hints, shown as hints only: they never decide (ADR-066 §2). */
function hintWords(t: AddOnTool): string | null {
  if (t.destructiveHint === true) return "The program says it may delete or overwrite things.";
  if (t.readOnlyHint === true) return "The program says it only reads.";
  if (t.readOnlyHint === false) return "The program says it changes things.";
  return null;
}

function AddOnCard({
  addOn: a,
  page,
  onApply,
}: {
  addOn: AddOn;
  page: ConnectionsPage;
  onApply: (page: ConnectionsPage) => void;
}) {
  const { pending, error, run } = useRun(onApply);
  // Looking at its tools starts the program, which takes a moment: its own "busy".
  const look = useRun(onApply);
  const busy = pending || look.pending;
  const [confirmRemove, setConfirmRemove] = useState(false);
  const titleId = `add-on-${a.id}`;
  const changed = a.tools.filter((t) => t.changed).length;
  const marked = a.tools.filter((t) => t.mark !== "off").length;
  // One line while closed (Phase 25, item 2.2): its tools in use, and who may use it.
  const summary = [
    a.tools.length > 0
      ? `${marked} of ${a.tools.length} tools in use`
      : "Its tools not looked at yet",
    whoMayUseWords(a.access.length),
  ].join(" · ");
  return (
    <li
      className={`connection connection--opens connection--${a.on ? "connected" : "notConnected"}`}
      aria-labelledby={titleId}
    >
      <Disclosure
        title={a.name}
        headingId={titleId}
        headingLevel={4}
        status={{ status: a.on ? "ok" : "offline", label: a.on ? "On" : "Off" }}
        summary={summary}
        openWhen={changed > 0}
        rememberAs={`add-on:${a.id}`}
      >
        <p className="muted">
          Program: <span className="path">{a.program}</span>
          {a.args.length > 0 && (
            <>
              {" "}
              with <span className="path">{a.args.join(" ")}</span>
            </>
          )}
          {a.secrets.length > 0 && <> · given the stored secrets {a.secrets.join(", ")}</>}
        </p>
        {changed > 0 && (
          <p className="notice-box" role="alert">
            <strong>
              {changed === 1 ? "1 tool changed" : `${changed} tools changed`} — look again.
            </strong>{" "}
            Its description or what it takes changed in the program since you marked it, so it is
            Off until you mark it again.
          </p>
        )}
        <div className="actions">
          <Button
            variant={a.on ? "secondary" : "primary"}
            size="sm"
            disabled={busy}
            onClick={() => void run(() => changeAddOn(a.id, { on: !a.on }))}
          >
            {a.on ? "Switch off" : "Switch on"}
          </Button>
          <Button
            variant="secondary"
            size="sm"
            disabled={busy}
            onClick={() => void look.run(() => checkAddOnTools(a.id))}
          >
            {look.pending ? "Starting it…" : "Look at its tools"}
          </Button>
          {confirmRemove ? (
            <>
              <span className="muted">Remove {a.name}? Its marks and its list go with it.</span>
              <Button
                variant="danger"
                size="sm"
                disabled={pending}
                onClick={() => void run(() => removeAddOn(a.id))}
              >
                Yes, remove
              </Button>
              <Button variant="quiet" size="sm" onClick={() => setConfirmRemove(false)}>
                Keep it
              </Button>
            </>
          ) : (
            <Button variant="danger" size="sm" onClick={() => setConfirmRemove(true)}>
              Remove
            </Button>
          )}
        </div>
        <Refusal error={error ?? look.error} />
        <section className="connection__section" aria-labelledby={`${a.id}-tools`}>
          <h5 id={`${a.id}-tools`}>Its tools</h5>
          {a.tools.length === 0 ? (
            <p className="empty">
              Not looked at yet. Switch it on, or press Look at its tools: Plenipo starts it once,
              lists its tools, and stops it.
            </p>
          ) : (
            <ul className="connection-parts">
              {a.tools.map((t) => (
                <li key={t.name} className="connection-part">
                  <div className="connection-part__head">
                    <strong>{t.name}</strong>
                    {t.changed && <StatusPill status="warn" label="Changed — look again" />}
                    <Segmented<ToolMark>
                      label={`${t.name}: what it may do`}
                      value={t.mark}
                      options={MARKS.map((m) => ({ value: m, label: MARK_LABEL[m] }))}
                      onChange={(mark) => {
                        if ((mark !== t.mark || t.changed) && !pending) {
                          void run(() => setAddOnTools(a.id, { [t.name]: mark }));
                        }
                      }}
                    />
                  </div>
                  <p className="connection-part__words">
                    The program&apos;s words: <q>{t.description || "(no description)"}</q>{" "}
                    {hintWords(t)}
                  </p>
                </li>
              ))}
            </ul>
          )}
          <p className="muted">
            Reading goes ahead for workers at Read only or more. Changing asks you every time, for
            workers at Read and write. Off: nobody sees it.
          </p>
        </section>
        <WhoMayUse
          id={`add-on-${a.id}`}
          access={a.access}
          people={page.people}
          onApply={onApply}
          onSave={(next) => changeAddOn(a.id, { access: next })}
          levelWords="Read only uses its Reading tools; Read and write adds its Changing tools."
          headingLevel={5}
        />
      </Disclosure>
    </li>
  );
}
