import { useState } from "react";
import type { AiToolsPage, LearningSnapshot, LedgerStatus } from "@plenipo/types";
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

function Lessons() {
  const { org } = useSession();
  const { data, error } = useRead<LearningSnapshot>({ kind: "readLessons", org }, ["lessons"]);
  if (error) return <p className="form-error">{error}</p>;
  if (!data) return <p role="status">Loading…</p>;
  if (data.waiting.length === 0) return <p className="muted">No lessons are waiting for you.</p>;
  return (
    <ul className="list">
      {data.waiting.map((l) => (
        <li key={l.id} className="card card--quiet">
          <strong>{l.worker}</strong>
          <p>{l.text}</p>
          <p className="muted">Keep or discard it on your PC.</p>
        </li>
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
