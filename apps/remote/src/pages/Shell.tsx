import { useState } from "react";
import type { ControlStatus } from "@plenipo/types";
import { Button, Icon, type IconName } from "@plenipo/ui";

import { useSession } from "../session-context";
import { ActivityPage } from "./Activity";
import { ApprovalsPage } from "./Approvals";
import { HomePage } from "./Home";
import { MorePage } from "./More";
import { PageGuard } from "./PageGuard";
import { WorkPage } from "./Work";

export type Tab = "home" | "approvals" | "work" | "activity" | "more";

const TABS: { id: Tab; label: string; icon: IconName }[] = [
  { id: "home", label: "Home", icon: "home" },
  { id: "approvals", label: "Approvals", icon: "approvals" },
  { id: "work", label: "Work", icon: "workers" },
  { id: "activity", label: "Activity", icon: "activity" },
  { id: "more", label: "More", icon: "more" },
];

/**
 * Stop all, the same as on your PC: browser, desktop, and server work stops, until you allow it
 * again on the PC.
 */
function StopAll() {
  const { ask } = useSession();
  const [asking, setAsking] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  if (said) {
    return (
      <p className="stop-all__said" role="status">
        {said}
      </p>
    );
  }
  if (!asking) {
    return (
      <Button size="sm" variant="danger" icon="stop" onClick={() => setAsking(true)}>
        Stop all
      </Button>
    );
  }
  return (
    <div className="stop-all__ask" role="alertdialog" aria-label="Stop all?">
      <span>Stop all browser, desktop, and server work on your PC?</span>
      <Button
        size="sm"
        variant="danger"
        onClick={() => {
          setAsking(false);
          ask({ kind: "stopAll" })
            .then((r) => {
              const stopped = (r.ok as ControlStatus | undefined)?.stopped;
              setSaid(
                stopped
                  ? "Browser, desktop, and server work is stopped. Allow it again on your PC."
                  : (r.refused?.message ?? r.failed ?? "Your PC did not stop."),
              );
            })
            .catch(() => setSaid("We don't know if your PC got this. Check again when it's back."));
        }}
      >
        Stop all
      </Button>
      <Button size="sm" onClick={() => setAsking(false)}>
        Cancel
      </Button>
    </div>
  );
}

/**
 * The phone's page once signed in: which organization, Stop all, and the pages along the bottom.
 * What a phone may do is a fixed list (ADR-145); the terminal, files, the screen, Plenipo's browser,
 * secrets, and every setting stay on your PC.
 */
export function Shell() {
  const { organizations, org, setOrg, kept, status, note, clearNote, unknown, connect } =
    useSession();
  const [tab, setTab] = useState<Tab>("home");
  const offline = status.kind === "offline";
  return (
    <div className="shell">
      <header className="shell__top">
        <div className="shell__pc">
          <Icon name="phone" size={16} />
          <span>{kept.paired.pcName}</span>
        </div>
        {organizations.length > 1 && (
          <label className="shell__org">
            <span className="ui-visually-hidden">Organization</span>
            <select value={org} onChange={(e) => setOrg(e.target.value)}>
              {organizations.map((o) => (
                <option key={o.id} value={o.id}>
                  {o.name}
                </option>
              ))}
            </select>
          </label>
        )}
        {!offline && <StopAll />}
      </header>
      {offline && (
        <div className="banner banner--offline" role="alert">
          <p>
            <strong>Your PC can&rsquo;t be reached. Nothing was changed.</strong>
            {unknown &&
              " We don't know if your PC got your last request. Check again when it's back."}
          </p>
          <Button size="sm" onClick={connect}>
            Try again
          </Button>
        </div>
      )}
      {note && (
        <div className="banner" role="status">
          <p>{note}</p>
          <Button size="sm" onClick={clearNote}>
            OK
          </Button>
        </div>
      )}
      <main className="shell__page" aria-live="polite">
        {!offline && (
          <PageGuard key={`${tab}:${org}`}>
            {tab === "home" && <HomePage go={setTab} />}
            {tab === "approvals" && <ApprovalsPage />}
            {tab === "work" && <WorkPage />}
            {tab === "activity" && <ActivityPage key={org} />}
            {tab === "more" && <MorePage />}
          </PageGuard>
        )}
      </main>
      <nav className="shell__tabs" aria-label="Pages">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            className="shell__tab"
            aria-current={tab === t.id ? "page" : undefined}
            onClick={() => setTab(t.id)}
          >
            <Icon name={t.icon} size={20} />
            <span>{t.label}</span>
          </button>
        ))}
      </nav>
    </div>
  );
}
