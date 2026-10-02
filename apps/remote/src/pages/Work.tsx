import { useId, useState, type FormEvent } from "react";
import type {
  AgentSessionDetail,
  OrgSnapshot,
  PositionInfo,
  Task,
  TaskBrief,
  TaskRecord,
  WorkRecord,
  WorkView,
} from "@plenipo/types";
import { Button, Segmented, StatusPill } from "@plenipo/ui";

import { useRead, useSession } from "../session-context";
import { STATE_WORDS, ago, describe } from "../words";

type View = "projects" | "workers" | "tasks";
type Open =
  | { kind: "project"; id: string; name: string }
  | { kind: "worker"; position: PositionInfo }
  | { kind: "task"; id: string }
  | { kind: "conversation"; id: string }
  | null;

/** The PC's own size limit for an objective. */
const MAX_OBJECTIVE_BYTES = 40_000;

/** A position that takes objectives: on the job full-time, with its own AI worker (as on the PC). */
function takesObjectives(p: PositionInfo): boolean {
  return p.active && p.staffing === "persistent" && p.agent !== null;
}

const UNKNOWN = "We don't know if your PC got this. Check again when it's back.";

/** The conversation an objective went to, from the PC's answer. */
function conversationOf(ok: unknown): string | null {
  if (typeof ok !== "object" || ok === null || !("conversation" in ok)) return null;
  return typeof ok.conversation === "string" ? ok.conversation : null;
}

/**
 * **Stop the worker**: its task stops now, as on your PC. What it already did stays. It asks
 * first.
 */
function StopTask({ conversation, onDone }: { conversation: string; onDone: () => void }) {
  const { ask, org } = useSession();
  const [asking, setAsking] = useState(false);
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  if (said) {
    return (
      <p className="muted" role="status">
        {said}
      </p>
    );
  }
  if (!asking) {
    return (
      <Button size="sm" variant="danger" icon="stop" onClick={() => setAsking(true)}>
        Stop the worker
      </Button>
    );
  }
  return (
    <div className="notice-box" role="alertdialog" aria-label="Stop the worker?">
      <p>Stop this task now? What the worker already did stays.</p>
      <div className="actions">
        <Button
          size="sm"
          variant="danger"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            ask({ kind: "stopTask", org, conversation })
              .then((r) => {
                setSaid(
                  r.ok !== undefined
                    ? "Stopped."
                    : (r.refused?.message ?? r.failed ?? "Your PC did not stop it."),
                );
                onDone();
              })
              .catch(() => setSaid(UNKNOWN))
              .finally(() => setBusy(false));
          }}
        >
          Stop the worker
        </Button>
        <Button size="sm" onClick={() => setAsking(false)}>
          Keep going
        </Button>
      </div>
    </div>
  );
}

/**
 * **Give objective** to a position, in words only: files stay on your PC (ADR-145). Its worker
 * starts on it at once, as when you give it on your PC.
 */
function GiveObjective({
  position,
  busy,
  onGiven,
}: {
  position: PositionInfo;
  busy: boolean;
  onGiven: (conversation: string | null) => void;
}) {
  const { ask, org } = useSession();
  const id = useId();
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const tooLong = new TextEncoder().encode(text).length > MAX_OBJECTIVE_BYTES;
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!text.trim() || tooLong) return;
    setSending(true);
    setSaid(null);
    ask({ kind: "sendObjective", org, position: position.id, text })
      .then((r) => {
        if (r.ok === undefined) {
          setSaid(r.refused?.message ?? r.failed ?? "Your PC did not take the objective.");
          return;
        }
        setText("");
        setSaid("Objective given. Follow it in its conversation.");
        onGiven(conversationOf(r.ok));
      })
      .catch(() => setSaid(UNKNOWN))
      .finally(() => setSending(false));
  };
  return (
    <form className="objective" aria-label="Give an objective" onSubmit={submit}>
      <label htmlFor={id}>Objective for {position.title}</label>
      <textarea
        id={id}
        rows={4}
        value={text}
        aria-describedby={`${id}-hint`}
        onChange={(e) => {
          setText(e.target.value);
          setSaid(null);
        }}
      />
      <p id={`${id}-hint`} className="muted">
        Say what it should get done. Words only: files stay on your PC.
        {tooLong ? " That is longer than your PC takes." : ""}
      </p>
      {busy && <p className="muted">Busy with its current objective; wait until it finishes.</p>}
      <div className="actions">
        <Button
          type="submit"
          variant="primary"
          disabled={sending || busy || !text.trim() || tooLong}
        >
          {sending ? "Sending…" : "Give objective"}
        </Button>
      </div>
      {said && <p role="status">{said}</p>}
    </form>
  );
}

function Problem({ error, reload }: { error: string; reload: () => void }) {
  return (
    <>
      <p className="form-error" role="alert">
        {error}
      </p>
      <Button onClick={reload}>Try again</Button>
    </>
  );
}

function Decisions({ record }: { record: WorkRecord }) {
  if (record.decisions.length === 0 && record.pullRequests.length === 0) {
    return <p className="muted">Nothing recorded yet.</p>;
  }
  return (
    <ul className="list">
      {record.pullRequests.map((p) => (
        <li key={p.url} className="card card--quiet">
          <strong>Pull request{p.number ? ` #${p.number}` : ""}</strong>
          <p className="muted">{p.url}</p>
        </li>
      ))}
      {record.decisions.map((e) => {
        const words = describe(e);
        return words ? (
          <li key={e.id} className="card card--quiet">
            <span>{words}</span>
            <p className="muted">{ago(e.createdAt)}</p>
          </li>
        ) : null;
      })}
    </ul>
  );
}

function Brief({
  t,
  onOpen,
  onStopped,
}: {
  t: TaskBrief;
  onOpen: (o: Open) => void;
  onStopped?: () => void;
}) {
  return (
    <li className="card">
      <div className="card__head">
        <strong>{t.objective}</strong>
        <StatusPill
          status={t.state === "running" ? "warn" : "offline"}
          label={STATE_WORDS[t.state]}
        />
      </div>
      <p className="muted">
        {t.positionTitle ?? ""} · {ago(t.createdAt)}
      </p>
      <div className="actions">
        <Button size="sm" onClick={() => onOpen({ kind: "task", id: t.id })}>
          The task
        </Button>
        {t.sessionId && (
          <Button size="sm" onClick={() => onOpen({ kind: "conversation", id: t.sessionId! })}>
            Its conversation
          </Button>
        )}
      </div>
      {t.sessionId && t.state === "running" && onStopped && (
        <StopTask conversation={t.sessionId} onDone={onStopped} />
      )}
    </li>
  );
}

function ProjectPage({ id, name }: { id: string; name: string }) {
  const { org } = useSession();
  const { data, error, reload } = useRead<WorkRecord>({ kind: "readProjects", org, project: id }, [
    "tasks",
  ]);
  return (
    <>
      <h2>{name}</h2>
      {error ? (
        <Problem error={error} reload={reload} />
      ) : data ? (
        <Decisions record={data} />
      ) : (
        <p role="status">Loading…</p>
      )}
    </>
  );
}

function WorkerPage({ position, onOpen }: { position: PositionInfo; onOpen: (o: Open) => void }) {
  const { org } = useSession();
  const { data, error, reload } = useRead<WorkView>(
    { kind: "readWorkers", org, position: position.id },
    ["tasks"],
  );
  if (error) return <Problem error={error} reload={reload} />;
  if (!data) return <p role="status">Loading…</p>;
  const sections: [string, TaskBrief[]][] = [
    ["Working now", data.running],
    ["Waiting", data.waiting],
    ["Waiting its turn", data.queued],
    ["Finished lately", data.recent],
  ];
  const busy = data.running.length > 0 || data.waiting.length > 0;
  return (
    <>
      <h2>{position.title}</h2>
      {takesObjectives(position) && (
        <GiveObjective
          position={position}
          busy={busy}
          onGiven={(conversation) => {
            reload();
            if (conversation) onOpen({ kind: "conversation", id: conversation });
          }}
        />
      )}
      {sections.map(([label, tasks]) =>
        tasks.length === 0 ? null : (
          <div key={label}>
            <h3>{label}</h3>
            <ul className="list">
              {tasks.map((t) => (
                <Brief key={t.id} t={t} onOpen={onOpen} onStopped={reload} />
              ))}
            </ul>
          </div>
        ),
      )}
    </>
  );
}

function TaskPage({ id }: { id: string }) {
  const { org } = useSession();
  const { data, error, reload } = useRead<TaskRecord>({ kind: "readTasks", org, task: id }, [
    "tasks",
    "approvals",
  ]);
  if (error) return <Problem error={error} reload={reload} />;
  if (!data) return <p role="status">Loading…</p>;
  return (
    <>
      <h2>The task</h2>
      {data.approvals.length > 0 && (
        <>
          <h3>Approvals</h3>
          <ul className="list">
            {data.approvals.map((a) => (
              <li key={a.id} className="card card--quiet">
                <strong>{a.summary}</strong>
                <p className="muted">{a.note ?? a.status}</p>
              </li>
            ))}
          </ul>
        </>
      )}
      <h3>What happened</h3>
      <Decisions record={data.record} />
    </>
  );
}

function ConversationPage({ id }: { id: string }) {
  const { org } = useSession();
  const { data, error, reload } = useRead<AgentSessionDetail>(
    { kind: "readTasks", org, conversation: id },
    ["tasks"],
  );
  if (error) return <Problem error={error} reload={reload} />;
  if (!data) return <p role="status">Loading…</p>;
  const going = data.turns.some((t) => t.running || t.waiting);
  return (
    <>
      <h2>{data.session.title || "Conversation"}</h2>
      {going && <StopTask conversation={id} onDone={reload} />}
      <ol className="list conversation">
        {data.turns.map((t) => (
          <li key={t.taskId} className="card">
            <p className="conversation__you">{t.objective}</p>
            {t.running && <p className="muted">Working…</p>}
            {t.waiting && <p className="muted">Waiting…</p>}
            {t.result && (
              <p className="conversation__answer">{t.result.text ?? t.result.summary}</p>
            )}
          </li>
        ))}
      </ol>
    </>
  );
}

/**
 * Work: the projects, the workers, and the tasks, each with its page. A worker's page can give it
 * an objective, and a working task can be stopped.
 */
export function WorkPage() {
  const { org } = useSession();
  const [view, setView] = useState<View>("workers");
  const [open, setOpen] = useState<Open>(null);
  const snapshot = useRead<OrgSnapshot>({ kind: "readOrganization", org }, [
    "tasks",
    "organizations",
  ]);
  const tasks = useRead<Task[]>(view === "tasks" ? { kind: "readTasks", org } : null, ["tasks"]);
  if (open) {
    return (
      <section className="page">
        <Button size="sm" icon="chevronLeft" onClick={() => setOpen(null)}>
          Back
        </Button>
        {open.kind === "project" && <ProjectPage id={open.id} name={open.name} />}
        {open.kind === "worker" && <WorkerPage position={open.position} onOpen={setOpen} />}
        {open.kind === "task" && <TaskPage id={open.id} />}
        {open.kind === "conversation" && <ConversationPage id={open.id} />}
      </section>
    );
  }
  const s = snapshot.data;
  return (
    <section className="page" aria-labelledby="work-title">
      <h1 id="work-title">Work</h1>
      <Segmented<View>
        label="Show"
        value={view}
        onChange={setView}
        options={[
          { value: "workers", label: "Workers" },
          { value: "projects", label: "Projects" },
          { value: "tasks", label: "Tasks" },
        ]}
      />
      {snapshot.error && <Problem error={snapshot.error} reload={snapshot.reload} />}
      {view === "workers" &&
        (s ? (
          <ul className="list">
            {s.positions
              .filter((p) => p.active)
              .map((p) => (
                <li key={p.id} className="card">
                  <div className="card__head">
                    <strong>{p.title}</strong>
                    <span className="muted">{p.roleName}</span>
                  </div>
                  {p.currentTask && <p>{p.currentTask.objective}</p>}
                  {p.statusDetail && <p className="muted">{p.statusDetail}</p>}
                  <Button size="sm" onClick={() => setOpen({ kind: "worker", position: p })}>
                    Their work
                  </Button>
                </li>
              ))}
          </ul>
        ) : (
          <p role="status">Loading…</p>
        ))}
      {view === "projects" &&
        (s ? (
          s.projects.filter((p) => p.active).length === 0 ? (
            <p className="muted">No projects yet.</p>
          ) : (
            <ul className="list">
              {s.projects
                .filter((p) => p.active)
                .map((p) => (
                  <li key={p.id} className="card">
                    <strong>{p.name}</strong>
                    {p.description && <p className="muted">{p.description}</p>}
                    <Button
                      size="sm"
                      onClick={() => setOpen({ kind: "project", id: p.id, name: p.name })}
                    >
                      Its page
                    </Button>
                  </li>
                ))}
            </ul>
          )
        ) : (
          <p role="status">Loading…</p>
        ))}
      {view === "tasks" &&
        (tasks.error ? (
          <Problem error={tasks.error} reload={tasks.reload} />
        ) : tasks.data ? (
          <ul className="list">
            {tasks.data.slice(0, 100).map((t) => (
              <li key={t.id} className="card">
                <div className="card__head">
                  <strong>{t.objective}</strong>
                  <StatusPill
                    status={t.state === "running" ? "warn" : "offline"}
                    label={STATE_WORDS[t.state]}
                  />
                </div>
                <p className="muted">{ago(t.createdAt)}</p>
                <Button size="sm" onClick={() => setOpen({ kind: "task", id: t.id })}>
                  The task
                </Button>
              </li>
            ))}
          </ul>
        ) : (
          <p role="status">Loading…</p>
        ))}
    </section>
  );
}
