import { useState, type FormEvent } from "react";
import type {
  CommandClass,
  Environment,
  HostKeyInput,
  ServerApproval,
  ServerIdentity,
  ServerInput,
  ServersSnapshot,
  ServerView,
  SignIn,
} from "@plenipo/types";

import { checkServerIdentity, saveServer, toCommandError } from "../../api/commands";
import { useRun } from "../../guard/useRun";
import {
  APPROVAL_LABEL,
  defaultClasses,
  ENVIRONMENT_HINT,
  ENVIRONMENT_LABEL,
  ENVIRONMENTS,
  lines,
  SIGN_IN_LABEL,
} from "../../servers/format";
import { Refusal } from "../models/shared";

/**
 * Add or change a server. Keys and passwords typed here go once to the operating system's
 * protected storage and are never shown again; leaving them empty keeps what is stored. The
 * server ID (its host key fingerprint) is read from the server and pinned only after you confirm it.
 */
export function ServerForm({
  snapshot,
  view,
  onApply,
  onDone,
}: {
  snapshot: ServersSnapshot;
  view: ServerView | null;
  onApply: (s: ServersSnapshot) => void;
  onDone: () => void;
}) {
  const s = view?.server;
  const [name, setName] = useState(s?.name ?? "");
  const [host, setHost] = useState(s?.host ?? "");
  const [port, setPort] = useState(String(s?.port ?? 22));
  const [user, setUser] = useState(s?.user ?? "");
  const [environment, setEnvironment] = useState<Environment>(s?.environment ?? "development");
  const [signIn, setSignIn] = useState<SignIn>(s?.signIn ?? "key");
  const [key, setKey] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [password, setPassword] = useState("");
  const [roles, setRoles] = useState<string[]>(s?.roles ?? []);
  const [classes, setClasses] = useState<CommandClass[]>(
    s?.classes ?? defaultClasses("development"),
  );
  const [approval, setApproval] = useState<ServerApproval>(s?.approval ?? "changes");
  const [folders, setFolders] = useState((s?.folders ?? []).join("\n"));
  const [forwards, setForwards] = useState((s?.forwards ?? []).join("\n"));
  const [pinned, setPinned] = useState<HostKeyInput | null>(
    s?.hostKey ? { algorithm: s.hostKey.algorithm, fingerprint: s.hostKey.fingerprint } : null,
  );
  const [seen, setSeen] = useState<ServerIdentity | null>(null);
  const [expected, setExpected] = useState("");
  const [checking, setChecking] = useState(false);
  const [checkError, setCheckError] = useState<string | null>(null);
  const { pending, error, run } = useRun((next: ServersSnapshot) => {
    onApply(next);
    onDone();
  });
  const production = environment === "production";
  const stored = view?.stored;

  const chooseEnvironment = (e: Environment) => {
    setEnvironment(e);
    if (!s) setClasses(defaultClasses(e));
    if (e === "production") setApproval("every");
  };
  const toggle = <T,>(list: T[], item: T, on: boolean) =>
    on ? [...list.filter((x) => x !== item), item] : list.filter((x) => x !== item);

  const check = async () => {
    setChecking(true);
    setCheckError(null);
    setSeen(null);
    try {
      setSeen(await checkServerIdentity(host.trim(), Number(port) || 22));
    } catch (e) {
      setCheckError(toCommandError(e).message);
    } finally {
      setChecking(false);
    }
  };
  const normalized = (fp: string) => fp.trim().replace(/=+$/, "");
  const mismatch =
    seen !== null &&
    expected.trim() !== "" &&
    normalized(expected) !== normalized(seen.fingerprint);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const input: ServerInput = {
      name,
      host,
      port: Number(port) || 22,
      user,
      environment,
      signIn,
      roles,
      classes,
      approval: production ? "every" : approval,
      folders: lines(folders),
      forwards: lines(forwards),
    };
    if (s) input.id = s.id;
    if (pinned) input.hostKey = pinned;
    if (signIn === "key" && key.trim()) input.key = key;
    if (signIn === "key" && passphrase) input.passphrase = passphrase;
    if (signIn === "password" && password) input.password = password;
    void run(() => saveServer(input));
  };

  const readKeyFile = (file: File | undefined) => {
    if (!file) return;
    void file.text().then(setKey);
  };

  return (
    <form
      className={`server-form server-form--${environment}`}
      aria-label={s ? `Change ${s.name}` : "Add a server"}
      onSubmit={submit}
    >
      <h4>{s ? `Change ${s.name}` : "Add a server"}</h4>
      <div className="server-form__row">
        <label className="field">
          <span>Name</span>
          <input
            value={name}
            placeholder="Website (production)"
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <label className="field">
          <span>Address</span>
          <input
            value={host}
            placeholder="web01.example.com or 203.0.113.10"
            onChange={(e) => setHost(e.target.value)}
          />
        </label>
        <label className="field field--narrow">
          <span>Port</span>
          <input inputMode="numeric" value={port} onChange={(e) => setPort(e.target.value)} />
        </label>
        <label className="field">
          <span>Sign in as</span>
          <input value={user} placeholder="deploy" onChange={(e) => setUser(e.target.value)} />
        </label>
      </div>

      <fieldset className="field">
        <legend>What it is</legend>
        {ENVIRONMENTS.map((env) => (
          <label key={env} className={`choice choice--${env}`}>
            <input
              type="radio"
              name="environment"
              value={env}
              checked={environment === env}
              onChange={() => chooseEnvironment(env)}
            />
            <span>
              <strong>{ENVIRONMENT_LABEL[env]}</strong> — {ENVIRONMENT_HINT[env]}
            </span>
          </label>
        ))}
      </fieldset>
      {production && (
        <p className="notice-box notice-box--danger" role="note">
          <strong>Production server.</strong> Every command waits for your approval, whatever else
          you choose below.
        </p>
      )}

      <fieldset className="field">
        <legend>How Plenipo signs in</legend>
        <select
          aria-label="How Plenipo signs in"
          value={signIn}
          onChange={(e) => setSignIn(e.target.value as SignIn)}
        >
          {(["key", "password", "agent"] as SignIn[]).map((m) => (
            <option key={m} value={m}>
              {SIGN_IN_LABEL[m]}
            </option>
          ))}
        </select>
        {signIn === "key" && (
          <>
            <label className="field">
              <span>
                Private key
                {stored?.key ? " (one is stored: paste a new one only to replace it)" : ""}
              </span>
              <textarea
                rows={4}
                spellCheck={false}
                autoComplete="off"
                value={key}
                placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
                onChange={(e) => setKey(e.target.value)}
              />
            </label>
            <label className="field">
              <span>Or choose the key file</span>
              <input type="file" onChange={(e) => readKeyFile(e.target.files?.[0])} />
            </label>
            <label className="field">
              <span>
                Passphrase (if the key has one)
                {stored?.passphrase ? " (one is stored)" : ""}
              </span>
              <input
                type="password"
                autoComplete="off"
                value={passphrase}
                onChange={(e) => setPassphrase(e.target.value)}
              />
            </label>
          </>
        )}
        {signIn === "password" && (
          <label className="field">
            <span>
              Password{stored?.password ? " (one is stored: type a new one only to change it)" : ""}
            </span>
            <input
              type="password"
              autoComplete="off"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </label>
        )}
        <p className="muted">
          {signIn === "agent"
            ? "Plenipo asks your SSH agent (Windows' OpenSSH Authentication Agent, or Pageant) to sign in with the keys it holds. The agent is never forwarded to the server."
            : `Kept in ${snapshot.vault.label}, and never shown again — not here, not to a worker, and not in anything Plenipo records. Never paste it into a chat or an objective.`}
        </p>
      </fieldset>

      <fieldset className="field">
        <legend>Its server ID</legend>
        <p className="muted">
          Its fingerprint, which Plenipo checks every time it connects, before it signs in. If it ever changes,
          workers are blocked from the server until you check and pin it again.
        </p>
        {pinned && (
          <p>
            Pinned: <code>{pinned.fingerprint}</code> ({pinned.algorithm})
          </p>
        )}
        <div className="actions">
          <button
            type="button"
            className="button button--small"
            disabled={checking || host.trim() === ""}
            onClick={() => void check()}
          >
            {checking ? "Checking…" : pinned ? "Check it again" : "Check the server ID"}
          </button>
        </div>
        <Refusal error={checkError} />
        {seen && (
          <div className="identity" aria-label="The server ID">
            <p>
              {seen.host}:{seen.port} shows the server ID <code>{seen.fingerprint}</code> (
              {seen.algorithm}).
            </p>
            <p className="muted">
              Compare it with the fingerprint your hosting provider shows, or with what{" "}
              <code>ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub</code> prints on the server.
              Pin it only if they match.
            </p>
            <label className="field">
              <span>The fingerprint you expect (optional, to compare)</span>
              <input value={expected} onChange={(e) => setExpected(e.target.value)} />
            </label>
            {mismatch ? (
              <p className="form-error" role="alert">
                They are different. Do not pin it: this may not be your server.
              </p>
            ) : (
              <button
                type="button"
                className="button button--small"
                disabled={pinned?.fingerprint === seen.fingerprint}
                onClick={() =>
                  setPinned({ algorithm: seen.algorithm, fingerprint: seen.fingerprint })
                }
              >
                {pinned?.fingerprint === seen.fingerprint
                  ? "Pinned"
                  : "This is my server: pin this ID"}
              </button>
            )}
          </div>
        )}
      </fieldset>

      <fieldset className="field">
        <legend>Who may use it</legend>
        {snapshot.roles.map((r) => (
          <label key={r.id} className="choice">
            <input
              type="checkbox"
              checked={roles.includes(r.id)}
              onChange={(e) => setRoles(toggle(roles, r.id, e.target.checked))}
            />
            <span>
              {r.name}
              {!r.canConnect && (
                <span className="muted">
                  {" "}
                  (its permission set does not include Connect to servers)
                </span>
              )}
            </span>
          </label>
        ))}
      </fieldset>

      <fieldset className="field">
        <legend>What kinds of commands it allows</legend>
        {snapshot.classes.map((c) => (
          <label key={c.class} className="choice">
            <input
              type="checkbox"
              checked={classes.includes(c.class)}
              onChange={(e) => setClasses(toggle(classes, c.class, e.target.checked))}
            />
            <span>
              <strong>{c.label}</strong> <span className="muted">— {c.examples}</span>
              {c.class === "destroy" && production && classes.includes("destroy") && (
                <span className="form-error">
                  {" "}
                  On a production server. Each one will still ask you.
                </span>
              )}
            </span>
          </label>
        ))}
        <p className="muted">
          Deleting, wiping, shutting down, and running as administrator always ask you, on every
          server. Reaching another computer from a server is never allowed.
        </p>
      </fieldset>

      <div className="server-form__row">
        <label className="field">
          <span>Folders commands run in and may change (one per line)</span>
          <textarea
            rows={3}
            value={folders}
            placeholder="/var/www/site"
            onChange={(e) => setFolders(e.target.value)}
          />
          <span className="muted">
            Commands run in the first one. With none, they run in the home folder of the user
            Plenipo signs in as.
          </span>
        </label>
        <label className="field">
          <span>When to ask you</span>
          <select
            aria-label="When to ask you"
            value={production ? "every" : approval}
            disabled={production}
            onChange={(e) => setApproval(e.target.value as ServerApproval)}
          >
            {(["every", "changes", "allowed"] as ServerApproval[]).map((a) => (
              <option key={a} value={a}>
                {APPROVAL_LABEL[a]}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Ports that may be forwarded (one per line, like localhost:5432)</span>
          <textarea rows={2} value={forwards} onChange={(e) => setForwards(e.target.value)} />
          <span className="muted">
            Off unless you list one. Each forward still asks you, with the worker&apos;s reason.
          </span>
        </label>
      </div>

      <div className="actions">
        <button type="submit" className="button" disabled={pending}>
          {s ? "Save the server" : "Add the server"}
        </button>
        <button type="button" className="button button--quiet" onClick={onDone}>
          Cancel
        </button>
      </div>
      <Refusal error={error} />
    </form>
  );
}
