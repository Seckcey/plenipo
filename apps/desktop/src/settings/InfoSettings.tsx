/**
 * Settings sections that show how things are: the AI tools and their sign-in, the organization's
 * departments and projects, where Plenipo keeps its files, a summary of Diagnostics, and About.
 */

import { useState } from "react";
import type { AppInfo, LedgerBackups, LedgerStatus, LocalPath, OrgSnapshot } from "@plenipo/types";
import {
  Button,
  EmptyState,
  ErrorState,
  LoadingState,
  Pip,
  PlenipoLogo,
  PropertyList,
  RowList,
  type RowItem,
  type Status,
} from "@plenipo/ui";

import { getLedgerStatus, getLocalPaths, listLedgerBackups, toCommandError } from "../api/commands";
import { AUTH_LABEL, notReadyHint, runtimeStatus } from "../agents/format";
import { useAgents } from "../agents/useAgents";
import type { Go } from "../components/views";
import { formatBytes } from "../ledger/format";
import { POSITION_STATUS } from "../org/cards";
import { STATUS_LABEL } from "../org/format";
import { rankName, titlesOf } from "../org/titles";
import { useOrganization } from "../org/useOrganization";
import { OrganizationsSetting } from "../orgs/OrganizationsSetting";
import { useLive } from "../pages/useLive";
import { count, when } from "../pages/words";
import { isActive } from "../runtime/store";
import { useRuntime } from "../runtime/useRuntime";
import { systemWords } from "../system/words";
import { DiagnosticsFileButton } from "../upkeep/BackupsPanel";

const TONE: Record<ReturnType<typeof runtimeStatus>["tone"], Status> = {
  ok: "ok",
  warn: "warn",
  bad: "error",
  muted: "offline",
};

/**
 * Settings → AI tools: each AI tool, whether it is installed and signed in (each opens its card on
 * the AI tools page), and the rules.
 */
export function AiToolsSettings({ go }: { go: Go }) {
  const { state, refresh } = useAgents();
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const recheck = async () => {
    setChecking(true);
    setError(null);
    try {
      await refresh();
    } catch (reason) {
      setError(toCommandError(reason).message);
    } finally {
      setChecking(false);
    }
  };
  const rows: RowItem[] = state.runtimes.map((r) => {
    const status = runtimeStatus(r);
    return {
      id: r.id,
      title: r.label,
      detail: r.ready
        ? `${r.providerLabel} · ${AUTH_LABEL[r.auth.state]}`
        : (notReadyHint(r) ?? r.providerLabel),
      status: { status: TONE[status.tone], label: status.text },
      meta: r.installation.version ?? undefined,
      // Its card on the AI tools page: sign in, usage, and updates.
      onOpen: () => go({ view: "runtimes", id: r.id }),
    };
  });
  return (
    <div className="settings-section__body">
      <div className="settings-section__actions">
        <Button size="sm" icon="refresh" onClick={() => void recheck()} disabled={checking}>
          {checking ? "Checking…" : "Check again"}
        </Button>
        <Button size="sm" variant="quiet" onClick={() => go({ view: "runtimes", id: null })}>
          Open the AI tools page
        </Button>
      </div>
      {error && (
        <p className="status status--error" role="alert">
          {error}
        </p>
      )}
      <RowList
        label="AI tools"
        items={rows}
        state={state.status === "error" && rows.length === 0 ? "error" : "ready"}
        error={state.error}
        onRetry={() => void recheck()}
        empty={
          <EmptyState
            compact
            title={
              state.status === "loading"
                ? "Looking for AI tools on this computer…"
                : "No AI tools found yet"
            }
          />
        }
      />
      <ul className="settings">
        <li>
          <strong>Billing:</strong> your subscriptions, and the paid AI keys you save, only while
          Let workers use paid AI keys is on (Settings → Switches) and only for positions you list
          them for. Every paid task is priced and recorded, within your spending caps if you set
          any. An AI tool signed in with its own API key outside Plenipo, or through a third-party
          cloud, is refused: its costs would skip your caps. Reaching a usage limit never moves work
          to another AI company.
        </li>
        <li>
          <strong>Permissions:</strong> workers never get their AI tool&apos;s own tools or your
          add-ons (MCP servers); Codex&apos;s own commands are switched off; it works through
          Plenipo&apos;s tools. Workers of your organization with permissions get Plenipo&apos;s own
          tools instead, confined to their project&apos;s folder and checked by Plenipo Guard. Tasks
          you start yourself in Workers get no tools.
        </li>
        <li>
          <strong>Your keys and secrets:</strong> API keys and other secrets on this computer are
          never passed to a worker. Only proxy settings and the tools&apos; own settings folders
          are. Secrets you store under Permissions go only to the programs you name, and are hidden
          wherever they would appear.
        </li>
        <li>
          <strong>Model:</strong> organization workers get the model their role&apos;s choices pick
          (Settings → AI models), unless you fixed an AI tool on the position. Tasks you start
          yourself in Workers use the AI tool&apos;s default unless you name a model.
        </li>
        <li>
          <strong>Handoffs:</strong> off unless you allow them for a new task. A worker may then ask
          a worker on another AI tool for help through Plenipo Liaison — workers never contact each
          other directly. Liaison records a sub-task, passes on only the context the worker chose
          (up to a limit), and brings the reply back to the same piece of work. Limits: 3 levels
          deep, 3 requests per answer, 8 reply rounds per task, 16 handoffs per piece of work. A
          worker&apos;s permissions come from your settings for its role and project; a request for
          more permissions is recorded but never grants anything.
        </li>
      </ul>
    </div>
  );
}

/** Settings → Organization: the departments and projects, each opening its page. */
export function OrganizationSettings({ go }: { go: Go }) {
  const { snapshot, status, error, reload } = useOrganization();
  if (!snapshot) {
    return status === "error" ? (
      <ErrorState
        title="Couldn't load the organization"
        message={error}
        onRetry={() => void reload()}
      />
    ) : (
      <LoadingState label="Loading the organization" />
    );
  }
  const t = titlesOf(snapshot);
  const lead = (org: OrgSnapshot, id: string | null) =>
    id ? org.positions.find((p) => p.id === id) : undefined;
  const departments: RowItem[] = snapshot.departments
    .filter((d) => !d.deleted)
    .map((d) => {
      const head = lead(snapshot, d.headPositionId);
      return {
        id: d.id,
        title: d.name,
        detail: [
          head
            ? `${rankName(t, "departmentManager")}: ${head.title}`
            : `No ${rankName(t, "departmentManager")} yet`,
          // A project deleted for good is only a short record: not counted.
          count(
            d.projectIds.filter((id) => snapshot.projects.some((x) => x.id === id && !x.deleted))
              .length,
            "project",
          ),
        ].join(" · "),
        status: !d.active
          ? { status: "offline", label: d.archivedAt !== null ? "Archived" : "Inactive" }
          : head
            ? { status: POSITION_STATUS[head.status], label: STATUS_LABEL[head.status] }
            : undefined,
        onOpen: () => go({ view: "department", id: d.id }),
      };
    });
  const projects: RowItem[] = snapshot.projects
    .filter((p) => !p.deleted)
    .map((p) => {
      const head = lead(snapshot, p.coordinatorPositionId);
      const department = snapshot.departments.find((d) => d.id === p.departmentId);
      return {
        id: p.id,
        title: p.name,
        detail: [
          department?.name ?? "No department",
          head
            ? `${rankName(t, "projectCoordinator")}: ${head.title}`
            : `No ${rankName(t, "projectCoordinator")} yet`,
          p.localPath ? "has a project folder" : "no project folder",
        ].join(" · "),
        status: p.active ? undefined : { status: "offline", label: "Archived" },
        onOpen: () => go({ view: "project", id: p.id }),
      };
    });
  return (
    <div className="settings-section__body">
      <div className="settings-section__actions">
        <Button
          size="sm"
          icon="organization"
          onClick={() => go({ view: "organization", id: null })}
        >
          Open the Organization map
        </Button>
      </div>
      <PropertyList
        items={[{ label: "Positions", value: count(snapshot.stats.positions, "position") }]}
      />
      <h3>Departments</h3>
      <RowList
        label="Departments"
        items={departments}
        empty={<EmptyState compact title="No departments yet" />}
      />
      <h3>Projects</h3>
      <RowList
        label="Projects"
        items={projects}
        empty={<EmptyState compact title="No projects yet" />}
      />
      <OrganizationsSetting />
    </div>
  );
}

/** Settings → Local paths: where Plenipo keeps its files (shown, never changed here). */
export function LocalPathsSettings() {
  const live = useLive<LocalPath[]>(
    "paths",
    () => getLocalPaths(),
    () => false,
  );
  if (live.status === "loading") return <LoadingState label="Loading the folders" />;
  if (live.status === "error" || !live.value) {
    return (
      <ErrorState title="Couldn't read the folders" message={live.error} onRetry={live.reload} />
    );
  }
  return (
    <div className="settings-section__body">
      <PropertyList
        items={live.value.map((p) => ({
          label: p.label,
          value: <code className="page__path">{p.path}</code>,
        }))}
      />
      <p className="muted">
        Plenipo keeps everything on this computer. Back up the Ledger&apos;s folder to keep your
        company&apos;s history; Diagnostics makes a backup or an export on request.
      </p>
    </div>
  );
}

/** Settings → Diagnostics: a summary, the programs Plenipo runs, and the raw details page. */
export function DiagnosticsSummary({ go, info }: { go: Go; info: AppInfo | null }) {
  const { state } = useRuntime();
  const agents = useAgents();
  const ledger = useLive<LedgerStatus>(
    "ledger",
    () => getLedgerStatus(),
    (e) => e.eventType.startsWith("ledger."),
  );
  const backups = useLive<LedgerBackups>(
    "backups",
    () => listLedgerBackups(),
    (e) => e.eventType.startsWith("ledger."),
  );
  const active = Object.values(state.executions).filter(isActive).length;
  const ready = agents.state.runtimes.filter((r) => r.ready).length;
  const l = ledger.value;
  const lastBackup = backups.value?.backups[0] ?? l?.lastBackup ?? null;
  return (
    <div className="settings-section__body">
      <div className="settings-section__actions">
        <Button size="sm" icon="diagnostics" onClick={() => go({ view: "diagnostics", id: null })}>
          Open Diagnostics (technical details)
        </Button>
      </div>
      <PropertyList
        state={ledger.status === "error" ? "error" : "ready"}
        error={ledger.error}
        onRetry={ledger.reload}
        items={[
          {
            label: "Version",
            value: info ? `${info.version} (${info.os} / ${info.arch})` : "Connecting…",
          },
          {
            label: "AI tools ready",
            value: `${ready} of ${agents.state.runtimes.length}`,
          },
          { label: "Programs running", value: String(active) },
          {
            label: "The Ledger",
            value: l
              ? `${l.persistent ? "Saved on this computer" : "Temporary (nothing is kept)"} · ${formatBytes(l.sizeBytes)} · ${count(l.taskCount, "task")}, ${count(l.eventCount, "event")}`
              : "Loading…",
          },
          {
            label: "Last check of the Ledger",
            value: l?.lastIntegrityCheck
              ? `${l.lastIntegrityCheck.ok ? "Healthy" : "Problems found"} · ${when(l.lastIntegrityCheck.checkedAt)}`
              : "Not checked yet",
          },
          {
            label: "Last backup",
            value: lastBackup ? when(lastBackup.createdAt) : "None yet",
          },
        ]}
      />
      <DiagnosticsFileButton />
      <h3>Programs Plenipo runs</h3>
      <ul className="settings">
        <li>
          <strong>Approved programs:</strong>{" "}
          {state.profiles.map((p) => p.label).join(", ") || "none"}
        </li>
        <li>
          <strong>Only these:</strong> Plenipo itself (built-in checks and its Ollama connection),
          the AI tools it finds (Claude Code, Codex, Grok, Kimi, Ollama, Antigravity, GitHub
          Copilot), and programs a worker runs with your permission (Settings → Permissions).
          Nothing on screen can supply a command, path, or argument.
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
    </div>
  );
}

/** Settings → About Plenipo: the logo, Pip, the version, and how Plenipo keeps you in charge. */
export function AboutPlenipo({ info }: { info: AppInfo | null }) {
  const words = systemWords();
  return (
    <div className="settings-about">
      <div className="settings-about__brand">
        <PlenipoLogo variant="square" height={148} />
        <Pip pose="welcome" size="lg" label="Pip, Plenipo's helper, waving hello" />
      </div>
      <PropertyList
        items={[
          { label: "Version", value: info ? info.version : "…" },
          { label: "License", value: "Elastic License 2.0" },
        ]}
      />
      <p>
        Plenipo runs a company of AI workers on this computer, with you as President. It uses your
        own signed-in AI tools, and your own paid keys if you add them, never sees your passwords,
        and asks you before anything that needs you.
      </p>
      <h3>Window behavior</h3>
      <ul className="settings">
        <li>
          Plenipo lives in {words.waitsIn}. Closing the window while work is going keeps the work
          running; use {words.waitsInIcon} to reopen Plenipo or stop the work. Settings → Start and
          close changes this.
        </li>
        <li>
          Quitting from {words.waitsIn} stops everything that is running and records how it ended.
        </li>
        <li>
          If Plenipo, or {words.theSystem}, stops unexpectedly, Plenipo tells you what stopped when
          it starts again, and nothing runs again until you choose Run again.
        </li>
      </ul>
    </div>
  );
}
