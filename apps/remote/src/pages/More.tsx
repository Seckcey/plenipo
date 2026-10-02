import { useState } from "react";
import type {
  AiToolsPage,
  LearningSnapshot,
  LedgerStatus,
  Lesson as LessonView,
} from "@plenipo/types";
import { Button, PropertyList, useTheme } from "@plenipo/ui";

import { useRead, useSession } from "../session-context";
import { aiToolStatus, type PhoneAiTool } from "../words";
import { NoticesOnThisPhone } from "./Notices";

/** The AI tools on your PC, by name, with what each can do now. */
function AiTools() {
  const { data, error } = useRead<AiToolsPage & { runtimes?: PhoneAiTool[] }>(
    { kind: "readAiTools" },
    ["aiTools"],
  );
  if (error) return <p className="form-error">{error}</p>;
  if (!data) return <p role="status">Loading…</p>;
  // A PC on 1.19.0 sends no names.
  if (!data.runtimes) {
    return <p className="muted">Update Plenipo on your PC to see its AI tools here.</p>;
  }
  const outOfService = new Map(data.tools.map((t) => [t.runtimeId, t.outOfService]));
  const onPc = data.runtimes.filter((r) => r.install !== "notInstalled");
  const notOnPc = data.runtimes.filter((r) => r.install === "notInstalled");
  return (
    <>
      {onPc.length === 0 ? (
        <p className="muted">No AI tool is installed on your PC yet.</p>
      ) : (
        <ul className="list">
          {onPc.map((r) => (
            <li key={r.id} className="card card--quiet">
              <strong>{r.label}</strong>
              <p className="muted">{aiToolStatus(r, outOfService.get(r.id) ?? null)}</p>
            </li>
          ))}
        </ul>
      )}
      {notOnPc.length > 0 && (
        <p className="muted">Not on your PC: {notOnPc.map((r) => r.label).join(", ")}.</p>
      )}
    </>
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
function Lesson({
  lesson,
  focused,
  onDone,
}: {
  lesson: LessonView;
  /** A notice opened the page on this one. */
  focused: boolean;
  onDone: () => void;
}) {
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
    <li className={focused ? "card card--quiet lesson--focused" : "card card--quiet"}>
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

function Lessons({ focus }: { focus: string | null }) {
  const { org } = useSession();
  const { data, error, reload } = useRead<LearningSnapshot>({ kind: "readLessons", org }, [
    "lessons",
  ]);
  if (error) return <p className="form-error">{error}</p>;
  if (!data) return <p role="status">Loading…</p>;
  // A notice opened the page on a lesson already kept or discarded (ADR-144 §8).
  const answered = focus !== null && !data.waiting.some((l) => l.id === focus);
  return (
    <>
      {answered && (
        <p className="notice-box" role="status">
          <strong>Already answered.</strong> That lesson was kept or discarded on your PC.
        </p>
      )}
      {data.waiting.length === 0 ? (
        <p className="muted">No lessons are waiting for you.</p>
      ) : (
        <ul className="list">
          {data.waiting.map((l) => (
            <Lesson key={l.id} lesson={l} focused={l.id === focus} onDone={reload} />
          ))}
        </ul>
      )}
    </>
  );
}

/** More: the AI tools, Diagnostics, lessons waiting, and this phone's own choices. */
export function MorePage({ focus = null }: { focus?: string | null }) {
  const { kept, leave } = useSession();
  const [theme, setTheme] = useTheme();
  const [removing, setRemoving] = useState(false);
  return (
    <section className="page" aria-labelledby="more-title">
      <h1 id="more-title">More</h1>
      <h2>Lessons waiting</h2>
      <Lessons focus={focus} />
      <h2>AI tools</h2>
      <AiTools />
      <h2>Diagnostics</h2>
      <Diagnostics />
      <h2>Notices on this phone</h2>
      <NoticesOnThisPhone />
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
