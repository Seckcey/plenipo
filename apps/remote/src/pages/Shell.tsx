import { useEffect, useState } from "react";
import type { ControlStatus, PhoneAsk } from "@plenipo/types";
import { Button, Icon, type IconName } from "@plenipo/ui";

import type { ControlRead } from "../control";
import { parseTarget } from "../notice";
import { subscribeOpen, takeInitialTarget } from "../open-target";
import { useRead, useSession } from "../session-context";
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
 * again. **Allow again** lets it go again, here or on your PC.
 */
function Control({ control, reload }: { control: ControlStatus | null; reload: () => void }) {
  const { ask } = useSession();
  const [asking, setAsking] = useState(false);
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const act = (request: PhoneAsk, wanted: boolean, refused: string) => {
    setAsking(false);
    setBusy(true);
    setSaid(null);
    ask(request)
      .then((r) => {
        const stopped = (r.ok as ControlStatus | undefined)?.stopped;
        if (stopped !== wanted) setSaid(r.refused?.message ?? r.failed ?? refused);
        reload();
      })
      .catch(() => setSaid("We don't know if your PC got this. Check again when it's back."))
      .finally(() => setBusy(false));
  };
  if (control?.stopped) {
    return (
      <div className="stop-all__said" role="status">
        <span>Browser, desktop, and server work is stopped.</span>
        <Button
          size="sm"
          variant="primary"
          icon="play"
          disabled={busy}
          onClick={() => act({ kind: "allowAgain" }, false, "Your PC did not allow work again.")}
        >
          Allow again
        </Button>
        {said && <span className="form-error">{said}</span>}
      </div>
    );
  }
  if (!asking) {
    return (
      <>
        <Button
          size="sm"
          variant="danger"
          icon="stop"
          disabled={busy}
          onClick={() => setAsking(true)}
        >
          Stop all
        </Button>
        {said && (
          <p className="stop-all__said form-error" role="alert">
            {said}
          </p>
        )}
      </>
    );
  }
  return (
    <div className="stop-all__ask" role="alertdialog" aria-label="Stop all?">
      <span>Stop all browser, desktop, and server work on your PC?</span>
      <Button
        size="sm"
        variant="danger"
        onClick={() => act({ kind: "stopAll" }, true, "Your PC did not stop.")}
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
/** Which page a tapped notice opens, and which item on it. */
function opened(target: string | null): { tab: Tab; focus: string | null; org: string | null } {
  if (!target) return { tab: "home", focus: null, org: null };
  const t = parseTarget(target);
  const tab: Tab =
    t.kind === "approval" || t.kind === "approvals" || t.kind === "checks"
      ? "approvals"
      : t.kind === "lesson" || t.kind === "lessons"
        ? "more"
        : t.kind === "finished" || t.kind === "problems"
          ? "work"
          : "home";
  return { tab, focus: t.id, org: t.org || null };
}

export function Shell() {
  const { organizations, org, setOrg, kept, status, note, clearNote, unknown, connect } =
    useSession();
  // A tapped notice opens its item (part 14C): once when the page opened on it, and again
  // whenever another is tapped while the page is open.
  const [first] = useState(() => opened(takeInitialTarget()));
  const [tab, setTab] = useState<Tab>(first.tab);
  const [focus, setFocus] = useState<string | null>(first.focus);
  useEffect(() => {
    if (first.org) setOrg(first.org);
  }, [first.org, setOrg]);
  useEffect(
    () =>
      subscribeOpen((target) => {
        const next = opened(target);
        if (next.org) setOrg(next.org);
        setTab(next.tab);
        setFocus(next.focus);
      }),
    [setOrg],
  );
  const offline = status.kind === "offline";
  const control = useRead<ControlRead>(offline ? null : { kind: "readControl" }, ["control"]);
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
        {!offline && <Control control={control.data?.control ?? null} reload={control.reload} />}
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
            {tab === "home" && (
              <HomePage go={setTab} control={control.data} reloadControl={control.reload} />
            )}
            {tab === "approvals" && <ApprovalsPage focus={focus} />}
            {tab === "work" && <WorkPage />}
            {tab === "activity" && <ActivityPage key={org} />}
            {tab === "more" && <MorePage focus={focus} />}
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
            onClick={() => {
              setTab(t.id);
              setFocus(null);
            }}
          >
            <Icon name={t.icon} size={20} />
            <span>{t.label}</span>
          </button>
        ))}
      </nav>
    </div>
  );
}
