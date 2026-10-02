import { useState } from "react";
import type {
  AiToolsPage,
  LearningSnapshot,
  LedgerStatus,
  Lesson as LessonView,
} from "@plenipo/types";
import { Button, PropertyList, useTheme } from "@plenipo/ui";

import { useRead, useSession } from "../session-context";

function AiTools() {
  const { data, error } = useRead<AiToolsPage>({ kind: "readAiTools" }, ["aiTools"]);
  if (error) return <p className="form-error">{error}</p>;
  if (!data) return <p role="status">Loading…</p>;
  return (
    <ul className="list">
      {data.tools.map((t) => (
        <li key={t.runtimeId} className="card card--quiet">
          <strong>{t.runtimeId}</strong>
          <p className="muted">
            {t.outOfService ?? (t.newest ? `Newest version: ${t.newest}` : "Ready")}
          </p>
        </li>
      ))}
    </ul>
  );
}

function Diagnostics() {
  const { data, error } = useRead<{ ledger: LedgerStatus; version: string }>({
    kind: "readDiagnostics",
  });
  if (error) return <p className="form-error">{error}</p>;
  if (!data) return <p role="status">Loading…</p>;
  return (
    <PropertyList
      items={[
        { label: "Plenipo on your PC", value: data.version },
        { label: "Tasks recorded", value: String(data.ledger.taskCount) },
        { label: "Events recorded", value: String(data.ledger.eventCount) },
        {
          label: "Last backup",
          value: data.ledger.lastBackup
            ? new Date(Number(data.ledger.lastBackup.createdAt)).toLocaleString()
            : "None yet",
        },
      ]}
    />
  );
}

/**
 * One lesson waiting for you: **Keep** it as written (workers in its role use it from then on), or
 * **Discard** it. Changing its words stays on your PC.
 */
function Lesson({ lesson, onDone }: { lesson: LessonView; onDone: () => void }) {
  const { ask, org } = useSession();
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const decide = (keep: boolean) => {
    setBusy(true);
    setSaid(null);
    ask({ kind: keep ? "keepLesson" : "discardLesson", org, lesson: lesson.id })
      .then((r) => {
        if (r.ok === undefined) {
          setSaid(r.refused?.message ?? r.failed ?? "Your PC did not take that answer.");
        }
        onDone();
      })
      .catch(() => setSaid("We don't know if your PC got this. Check again when it's back."))
      .finally(() => setBusy(false));
  };
  return (
    <li className="card card--quiet">
      <strong>{lesson.worker}</strong>
      <p>{lesson.text}</p>
      {lesson.heldReason && <p className="muted">{lesson.heldReason}</p>}
      <div className="actions">
        <Button
          size="sm"
          variant="primary"
          icon="check"
          disabled={busy}
          onClick={() => decide(true)}
        >
          Keep
        </Button>
        <Button size="sm" disabled={busy} onClick={() => decide(false)}>
          Discard
        </Button>
      </div>
      <p className="muted">To change its words first, open it on your PC.</p>
      {said && (
        <p className="form-error" role="alert">
          {said}
        </p>
      )}
    </li>
  );
}

function Lessons() {
  const { org } = useSession();
  const { data, error, reload } = useRead<LearningSnapshot>({ kind: "readLessons", org }, [
    "lessons",
  ]);
  if (error) return <p className="form-error">{error}</p>;
  if (!data) return <p role="status">Loading…</p>;
  if (data.waiting.length === 0) return <p className="muted">No lessons are waiting for you.</p>;
  return (
    <ul className="list">
      {data.waiting.map((l) => (
        <Lesson key={l.id} lesson={l} onDone={reload} />
      ))}
    </ul>
  );
}

/** More: the AI tools, Diagnostics, lessons waiting, and this phone's own choices. */
export function MorePage() {
  const { kept, leave } = useSession();
  const [theme, setTheme] = useTheme();
  const [removing, setRemoving] = useState(false);
  return (
    <section className="page" aria-labelledby="more-title">
      <h1 id="more-title">More</h1>
      <h2>Lessons waiting</h2>
      <Lessons />
      <h2>AI tools</h2>
      <AiTools />
      <h2>Diagnostics</h2>
      <Diagnostics />
      <h2>This phone</h2>
      <PropertyList
        items={[
          { label: "Paired with", value: kept.paired.pcName },
          { label: "This page", value: __PLENIPO_VERSION__ },
        ]}
      />
      <div className="actions">
        <Button
          icon={theme === "dark" ? "sun" : "moon"}
          onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
        >
          {theme === "dark" ? "Switch to the light theme" : "Switch to the dark theme"}
        </Button>
        <Button onClick={() => void leave("signOut")}>Sign out</Button>
      </div>
      {removing ? (
        <div className="notice-box" role="alertdialog" aria-label="Remove this phone?">
          <p>
            Remove this phone from your PC? It is cut off at once. To use it again, add it again on
            your PC.
          </p>
          <div className="actions">
            <Button variant="danger" onClick={() => void leave("remove")}>
              Remove this phone
            </Button>
            <Button onClick={() => setRemoving(false)}>Keep it</Button>
          </div>
        </div>
      ) : (
        <Button variant="danger" onClick={() => setRemoving(true)}>
          Remove this phone
        </Button>
      )}
      <p className="muted about">
        Plenipo is made by 8 West Ventures, LLC. Your PC stays in charge: Guard decides every
        request from this phone, and the terminal, files, the screen, Plenipo&rsquo;s browser, your
        secrets, and every setting stay on your PC.
      </p>
    </section>
  );
}
