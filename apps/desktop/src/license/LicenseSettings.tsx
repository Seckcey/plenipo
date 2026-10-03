import { useState } from "react";
import type { LicenseView } from "@plenipo/types";
import { Button, ErrorState, LoadingState, PropertyList, StatusPill } from "@plenipo/ui";

import {
  checkLicenseNow,
  enterLicenseKey,
  removeLicenseKey,
  toCommandError,
} from "../api/commands";
import { when } from "../pages/words";
import { useLicense } from "./useLicense";
import {
  CHECK_HOST,
  WHERE_TO_BUY,
  day,
  editionName,
  organizationsLine,
  planLine,
  reasonWords,
} from "./words";
import { sentenceStart, systemWords } from "../system/words";

type Busy = "enter" | "check" | "remove" | null;

/** The last check, for the list: "9:41 AM", or "Not yet". */
function lastCheckWords(view: LicenseView): string {
  if (view.lastChecked) return when(view.lastChecked);
  return view.lastTried ? "Not yet (Plenipo keeps trying)" : "Not yet";
}

/**
 * Settings → License (Phase 11A, ADR-021, ADR-022): Free or Pro and why, the key's ID (never the
 * key), the weekly check with 8 West, and entering, replacing, or removing a key. The key is typed
 * only here and kept in the Vault.
 */
export function LicenseSettings() {
  const { view: live, error: loadError, reload, show } = useLicense();
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState<Busy>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);

  if (!live) {
    return loadError ? (
      <ErrorState title="Couldn't load your license" message={loadError} onRetry={reload} />
    ) : (
      <LoadingState label="Loading your license" />
    );
  }
  const view: LicenseView = live;
  const hasKey = view.keyId !== null;

  const act = async (what: Busy, work: () => Promise<LicenseView>) => {
    setBusy(what);
    setError(null);
    try {
      show(await work());
      return true;
    } catch (reason) {
      setError(toCommandError(reason).message);
      return false;
    } finally {
      setBusy(null);
    }
  };

  const pill =
    view.edition === "pro" ? (
      <StatusPill status="ok" label={editionName(view)} />
    ) : (
      <StatusPill status={hasKey ? "warn" : "offline"} label="Free" />
    );

  return (
    <div className="settings-section__body settings-license">
      {view.testBuild && (
        <p className="notice-box" role="note">
          This copy of Plenipo was built for testing. It also accepts 8 West&apos;s test keys, which
          a released copy never does.
        </p>
      )}
      <div className="settings-license__edition">
        {pill}
        <p>{reasonWords(view)}</p>
      </div>
      {view.clockAheadDays !== null && (
        <p className="notice-box" role="note">
          <strong>
            {sentenceStart(systemWords().thisComputer)}&apos;s clock is {view.clockAheadDays} days
            ahead of 8 West&apos;s.
          </strong>{" "}
          Set the right date and time on {systemWords().thisComputer}, then choose{" "}
          <strong>Check now</strong>.
        </p>
      )}
      {view.problem && view.edition === "pro" && (
        <p className="muted" role="status">
          The last check didn&apos;t go through: {view.problem}
        </p>
      )}
      {view.problem && view.edition === "free" && hasKey && (
        <p className="muted" role="status">
          Why: {view.problem}
          {view.reason === "noCheck" && (
            <>
              {" "}
              Plenipo needs to reach <strong>{CHECK_HOST}</strong>. On a work network, ask whoever
              runs it to allow that address.
            </>
          )}
        </p>
      )}
      {view.problem && !hasKey && (
        <p className="status status--error" role="alert">
          {view.problem}
        </p>
      )}
      {hasKey && (
        <PropertyList
          items={[
            { label: "Now", value: editionName(view) },
            ...(view.holder ? [{ label: "Licensed to", value: view.holder }] : []),
            ...(planLine(view) ? [{ label: "Plan", value: planLine(view) }] : []),
            { label: "Organizations", value: organizationsLine(view) },
            ...(view.paidThrough ? [{ label: "Paid through", value: day(view.paidThrough) }] : []),
            ...(view.endsAt ? [{ label: "Pro ends", value: day(view.endsAt) }] : []),
            { label: "Last check with 8 West", value: lastCheckWords(view) },
            ...(view.nextCheck ? [{ label: "Next check", value: when(view.nextCheck) }] : []),
            ...(view.graceEnds && view.edition === "pro"
              ? [{ label: "Pro stays on without a check until", value: day(view.graceEnds) }]
              : []),
            { label: "Key ID", value: <span className="path">{view.keyId}</span> },
          ]}
        />
      )}
      {hasKey && (
        <div className="settings-section__actions">
          <Button
            size="sm"
            icon="refresh"
            disabled={busy !== null}
            onClick={() => void act("check", checkLicenseNow)}
          >
            {busy === "check" ? "Checking…" : "Check now"}
          </Button>
          <Button
            size="sm"
            disabled={busy !== null}
            onClick={() => {
              setError(null);
              setConfirmRemove(true);
            }}
          >
            Remove the key
          </Button>
        </div>
      )}
      {confirmRemove && (
        <div className="notice-box" role="note">
          <p>
            <strong>Remove the license key?</strong> Plenipo goes back to Free now. Nothing you made
            is deleted, and your subscription is not cancelled: enter the key again to turn Pro back
            on.
          </p>
          <div className="settings-section__actions">
            <Button
              size="sm"
              variant="danger"
              disabled={busy !== null}
              onClick={() => {
                setConfirmRemove(false);
                void act("remove", removeLicenseKey);
              }}
            >
              {busy === "remove" ? "Removing…" : "Remove the key"}
            </Button>
            <Button size="sm" onClick={() => setConfirmRemove(false)}>
              Keep it
            </Button>
          </div>
        </div>
      )}
      <form
        className="settings-license__enter"
        aria-labelledby="license-enter"
        onSubmit={(ev) => {
          ev.preventDefault();
          void act("enter", () => enterLicenseKey(key.trim())).then((ok) => {
            if (ok) setKey("");
          });
        }}
      >
        <h3 id="license-enter">{hasKey ? "Replace the key" : "Enter a license key"}</h3>
        <label className="field">
          <span>License key</span>
          <input
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={key}
            placeholder="plenipo1.…"
            onChange={(e) => setKey(e.target.value)}
          />
          <small className="muted">
            Paste the whole key from the email 8 West sent you. It is checked on{" "}
            {systemWords().thisComputer}, kept in the Vault, and never shown again. Pro starts at
            once, with no restart.
          </small>
        </label>
        <div className="settings-section__actions">
          <Button
            size="sm"
            variant="primary"
            type="submit"
            disabled={busy !== null || key.trim() === ""}
          >
            {busy === "enter" ? "Checking the key…" : hasKey ? "Replace the key" : "Enter the key"}
          </Button>
        </div>
      </form>
      {error && (
        <p className="status status--error" role="alert">
          {error}
        </p>
      )}
      <h3>How the license works</h3>
      <ul className="settings">
        <li>
          <strong>A Free copy never contacts 8 West.</strong> With no key, Plenipo sends nothing
          about you anywhere.
        </li>
        <li>
          With a key, Plenipo checks with 8 West about once a week. The check sends two things: the
          key ID above, and which version of Plenipo this is. Never your projects, files, or work.
        </li>
        <li>
          No internet is fine: Pro keeps working for 30 days between checks. If a check fails, Pro
          stays on and Plenipo tries again later. Without a check, Pro never runs more than 30 days
          past your paid-through date.
        </li>
        <li>
          If Pro ends, nothing you made is taken away. Every organization, department, project, and
          record stays; only making more than Free allows waits for Pro.
        </li>
        <li>
          Safety is never part of Pro: permissions, approvals, the Vault, the Ledger, and the
          Activity trail work the same on Free. Buy or manage Pro at {WHERE_TO_BUY}. Plenipo is made
          by 8 West Ventures, LLC.
        </li>
      </ul>
    </div>
  );
}
