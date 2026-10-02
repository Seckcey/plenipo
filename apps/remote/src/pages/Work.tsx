import { useState } from "react";
import type {
  AgentSessionDetail,
  OrgSnapshot,
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
  | { kind: "worker"; id: string; name: string }
  | { kind: "task"; id: string }
  | { kind: "conversation"; id: string }
  | null;

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
      {record.decisions.map((e) => (
        <li key={e.id} className="card card--quiet">
          <span>{describe(e)}</span>
          <p className="muted">{ago(e.createdAt)}</p>
        </li>
      ))}
    </ul>
  );
}

function Brief({ t, onOpen }: { t: TaskBrief; onOpen: (o: Open) => void }) {
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

function WorkerPage({ id, name, onOpen }: { id: string; name: string; onOpen: (o: Open) => void }) {
  const { org } = useSession();
  const { data, error, reload } = useRead<WorkView>({ kind: "readWorkers", org, position: id }, [
    "tasks",
  ]);
  if (error) return <Problem error={error} reload={reload} />;
  if (!data) return <p role="status">Loading…</p>;
  const sections: [string, TaskBrief[]][] = [
    ["Working now", data.running],
    ["Waiting", data.waiting],
    ["Waiting its turn", data.queued],
    ["Finished lately", data.recent],
  ];
  return (
    <>
      <h2>{name}</h2>
      {sections.map(([label, tasks]) =>
        tasks.length === 0 ? null : (
          <div key={label}>
            <h3>{label}</h3>
            <ul className="list">
              {tasks.map((t) => (
                <Brief key={t.id} t={t} onOpen={onOpen} />
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
  return (
    <>
      <h2>{data.session.title || "Conversation"}</h2>
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

/** Work: the projects, the workers, and the tasks, each with its page. Reading only. */
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
        {open.kind === "worker" && <WorkerPage id={open.id} name={open.name} onOpen={setOpen} />}
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
                  <Button
                    size="sm"
                    onClick={() => setOpen({ kind: "worker", id: p.id, name: p.title })}
                  >
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
