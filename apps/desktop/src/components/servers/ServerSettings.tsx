import { useState } from "react";
import type { Environment, ServersSnapshot, ServerTest, ServerView } from "@plenipo/types";

import { removeServer, testServer, toCommandError } from "../../api/commands";
import { useRun } from "../../guard/useRun";
import { ago } from "../../org/format";
import { useNow } from "../../runtime/useNow";
import { approvalWords, ENVIRONMENT_LABEL, SIGN_IN_PHRASE } from "../../servers/format";
import { useServers } from "../../servers/useServers";
import { Refusal } from "../models/shared";
import { ServerForm } from "./ServerForm";

/** Test, staging, or production — production always in red, in capitals. */
export function EnvironmentBadge({ environment }: { environment: Environment }) {
  return (
    <span className={`env env--${environment}`}>
      {environment === "production"
        ? ENVIRONMENT_LABEL.production.toUpperCase()
        : ENVIRONMENT_LABEL[environment]}
    </span>
  );
}

/**
 * Settings → Servers (Phase 11): the servers workers may reach over SSH. Each has a friendly
 * name, its address, how Plenipo signs in (a key or password kept in the operating system's
 * protected storage, or your SSH agent), its pinned server ID, test, staging, or
 * production, who may use it, what kinds of commands it allows, in which folders, and when you
 * are asked.
 */
export function ServerSettings() {
  const servers = useServers();
  const [editing, setEditing] = useState<ServerView | "new" | null>(null);
  const s = servers.snapshot;
  if (!s) {
    return (
      <p
        className={servers.error ? "form-error" : "muted"}
        role={servers.error ? "alert" : undefined}
      >
        {servers.error ?? "Loading the servers…"}
      </p>
    );
  }
  return (
    <div className="servers" aria-label="Servers">
      <p className="muted">
        Workers whose role may <em>Connect to servers</em> run commands over SSH only on the servers
        listed here, only if the server lists their role, and only the kinds of commands it allows.
        Plenipo checks each server&apos;s ID every time it connects. Keys and passwords are kept in{" "}
        {s.vault.label}: workers never see them, and neither does anything Plenipo records.
      </p>
      {!s.switchedOn && (
        <p className="notice-box notice-box--danger" role="note">
          <strong>Remote computers (SSH) are switched off</strong> in Settings → Switches, so no
          worker connects to any server. You can still add servers and test them here; turn the
          switch on when you want workers to use them.
        </p>
      )}
      <p className="notice-box" role="note">
        <strong>Production servers are marked in red.</strong> Every command on one waits for your
        approval, and deleting, wiping, or shutting down is blocked there unless you turn it on —
        and then it still asks. Workers can never connect from a server to another computer, and
        port forwarding is off unless you list a port.
      </p>
      {!s.vault.available && (
        <p className="form-error" role="alert">
          {s.vault.label} is not available on this computer
          {s.vault.detail ? `: ${s.vault.detail}` : "."} Keys and passwords cannot be stored; a
          server can still use your SSH agent.
        </p>
      )}
      {s.notices.length > 0 && (
        <ul className="notices">
          {s.notices.map((n) => (
            <li key={n}>{n}</li>
          ))}
        </ul>
      )}
      <div className="section-header">
        <h3 id="servers-title">Your servers</h3>
        <button type="button" className="button button--small" onClick={() => setEditing("new")}>
          Add a server
        </button>
      </div>
      {editing !== null && (
        <ServerForm
          key={editing === "new" ? "new" : editing.server.id}
          snapshot={s}
          view={editing === "new" ? null : editing}
          onApply={servers.apply}
          onDone={() => setEditing(null)}
        />
      )}
      {s.servers.length === 0 ? (
        <p className="empty">
          No servers yet. Add one, check its server ID, and choose which roles may use it.
        </p>
      ) : (
        <ul className="server-list" aria-labelledby="servers-title">
          {s.servers.map((v) => (
            <ServerCard
              key={v.server.id}
              view={v}
              snapshot={s}
              onApply={servers.apply}
              onEdit={() => setEditing(v)}
            />
          ))}
        </ul>
      )}
    </div>
  );
}

function ServerCard({
  view: v,
  snapshot,
  onApply,
  onEdit,
}: {
  view: ServerView;
  snapshot: ServersSnapshot;
  onApply: (s: ServersSnapshot) => void;
  onEdit: () => void;
}) {
  const s = v.server;
  const now = useNow(30_000);
  const { pending, error, run } = useRun(onApply);
  const [test, setTest] = useState<ServerTest | null>(null);
  const [testing, setTesting] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const roleName = (id: string) =>
    snapshot.roles.find((r) => r.id === id)?.name ?? "a removed role";
  const classLabel = (c: string) => snapshot.classes.find((x) => x.class === c)?.label ?? c;
  const stored =
    s.signIn === "key"
      ? v.stored.key
        ? `stored in ${snapshot.vault.label}${v.stored.passphrase ? ", with its passphrase" : ""}`
        : "not stored"
      : s.signIn === "password"
        ? v.stored.password
          ? `stored in ${snapshot.vault.label}`
          : "not stored"
        : "it signs in for Plenipo, and is never forwarded to the server";
  const runTest = async () => {
    setTesting(true);
    setTest(null);
    try {
      setTest(await testServer(s.id));
    } catch (e) {
      setTest({ ok: false, message: toCommandError(e).message });
    } finally {
      setTesting(false);
    }
  };
  return (
    <li
      className={`server server--${s.environment}`}
      aria-label={`${s.name}, ${ENVIRONMENT_LABEL[s.environment].toLowerCase()} server`}
    >
      <div className="server__header">
        <h4>{s.name}</h4>
        <EnvironmentBadge environment={s.environment} />
        <span className="path">{v.address}</span>
        {v.connected.length > 0 && (
          <span className="pill pill--warn">Connected: {v.connected.join(", ")}</span>
        )}
      </div>
      {v.identityChanged && (
        <p className="form-error" role="alert">
          <strong>This server&apos;s ID changed</strong> {ago(v.identityChanged.at, now)}: it showed
          the server ID <code>{v.identityChanged.fingerprint}</code> ({v.identityChanged.algorithm}
          ), not the one you pinned. Plenipo did not sign in, and workers are blocked from it. If
          you know why (for example, the server was reinstalled), check the new server ID with your
          hosting provider and pin it with <em>Change</em>. Otherwise, treat it as a possible
          attack.
        </p>
      )}
      {v.problem && !v.identityChanged && <p className="form-error">{v.problem}</p>}
      <dl className="server__facts">
        <dt>Signs in as</dt>
        <dd>
          {s.user}, with {SIGN_IN_PHRASE[s.signIn]} ({stored})
        </dd>
        <dt>Server ID</dt>
        <dd>
          {s.hostKey ? (
            <>
              <code>{s.hostKey.fingerprint}</code> ({s.hostKey.algorithm}), pinned{" "}
              {ago(s.hostKey.pinnedAt, now)}
            </>
          ) : (
            "Not pinned yet"
          )}
        </dd>
        <dt>Who may use it</dt>
        <dd>{s.roles.length > 0 ? s.roles.map(roleName).join(", ") : "No role yet"}</dd>
        <dt>Allows</dt>
        <dd>{s.classes.length > 0 ? s.classes.map(classLabel).join("; ") : "Nothing"}</dd>
        <dt>Commands run in</dt>
        <dd>{s.folders.length > 0 ? s.folders.join(", ") : `the home folder of ${s.user}`}</dd>
        <dt>Asks you</dt>
        <dd>{approvalWords(s.environment, s.approval)}</dd>
        <dt>Port forwarding</dt>
        <dd>{s.forwards.length > 0 ? `${s.forwards.join(", ")} (asks each time)` : "Off"}</dd>
      </dl>
      {test && (
        <p className={test.ok ? "server__test server__test--ok" : "form-error"} role="status">
          {test.message}
        </p>
      )}
      <div className="actions">
        <button
          type="button"
          className="button button--small"
          disabled={testing || !s.hostKey}
          onClick={() => void runTest()}
        >
          {testing ? "Testing…" : "Test the connection"}
        </button>
        <button
          type="button"
          className="button button--small button--quiet"
          aria-label={`Change ${s.name}`}
          onClick={onEdit}
        >
          Change
        </button>
        {confirmRemove ? (
          <>
            <span className="muted">Remove {s.name} and its stored sign-in?</span>
            <button
              type="button"
              className="button button--small button--danger"
              disabled={pending}
              onClick={() => void run(() => removeServer(s.id))}
            >
              Yes, remove it
            </button>
            <button
              type="button"
              className="button button--small button--quiet"
              onClick={() => setConfirmRemove(false)}
            >
              Keep it
            </button>
          </>
        ) : (
          <button
            type="button"
            className="button button--small button--danger"
            aria-label={`Remove ${s.name}`}
            onClick={() => setConfirmRemove(true)}
          >
            Remove
          </button>
        )}
      </div>
      <Refusal error={error} />
    </li>
  );
}
