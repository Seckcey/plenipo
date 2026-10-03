import { useId, useState, type FormEvent } from "react";
import type { CommunityView, MemberView, ProfileDraft } from "@plenipo/types";
import { Button, Checkbox, ErrorState, LoadingState, Select } from "@plenipo/ui";

import {
  cancelCommunitySignIn,
  checkCommunityAgain,
  joinCommunity,
  openCommunityPage,
  saveCommunityProfile,
  setCommunityAppearOffline,
  setCommunitySwitch,
  signOutOfCommunity,
  toCommandError,
} from "../api/commands";
import type { Go } from "../components/views";
import { sentenceStart, systemWords } from "../system/words";
import { BlockedPeople } from "./BlockedPeople";
import { DeleteMyData } from "./DeleteMyData";
import { LeaveCommunityDialog } from "./LeaveCommunity";
import { onDate } from "./messageWords";
import { sameProfile } from "./profileWords";
import { oneLine } from "./safeText";
import { useCommunity } from "./useCommunity";
import { WhatPeopleSee } from "./WhatPeopleSee";

const NOT_REACHED = "Community can't be reached right now. Nothing was changed.";

/** The fewest and the most letters, numbers, and dashes in a Community name. */
const LEAST_NAME = 3;
const MOST_NAME = 30;

const MONTHS = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
] as const;

/** What you type for a Community name: small letters, numbers, and dashes only. */
function cleanName(typed: string): string {
  return typed.toLowerCase().replace(/[^a-z0-9-]/g, "");
}

/**
 * The form to join Community: your name in it, your birth month and year (asked once, never the
 * day), and the terms. Nothing is sent until you press Join Community.
 */
function JoinForm({
  terms,
  profile,
  busy,
  onJoin,
  onTerms,
}: {
  /** The terms version to agree to, as 8 West names it. */
  terms: string | null;
  /** Your profile to begin with, every box ticked (ADR-163 §2). */
  profile: ProfileDraft;
  busy: boolean;
  onJoin: (name: string, month: number, year: number, terms: string, profile: ProfileDraft) => void;
  onTerms: () => void;
}) {
  const nameId = useId();
  const [name, setName] = useState("");
  const [month, setMonth] = useState("");
  const [year, setYear] = useState("");
  const [agreed, setAgreed] = useState(false);
  const [draft, setDraft] = useState(profile);
  const thisYear = new Date().getFullYear();
  const years = Array.from({ length: thisYear - 1900 + 1 }, (_, i) => String(thisYear - i));
  const ready =
    name.length >= LEAST_NAME && month !== "" && year !== "" && agreed && terms !== null;
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (ready && terms !== null) onJoin(name, Number(month), Number(year), terms, draft);
  };
  return (
    <form className="community-join" aria-label="Join Community" onSubmit={submit}>
      <div className="ui-field">
        <label htmlFor={nameId}>Your name in Community</label>
        <div className="community-join__name">
          <span aria-hidden="true">@</span>
          <input
            id={nameId}
            value={name}
            maxLength={MOST_NAME}
            autoComplete="off"
            spellCheck={false}
            aria-describedby={`${nameId}-hint`}
            onChange={(e) => setName(cleanName(e.target.value))}
          />
        </div>
        <div id={`${nameId}-hint`} className="ui-field__hint">
          3 to 30 letters, numbers, or dashes. People can find you by it.
        </div>
      </div>
      <fieldset className="community-join__birth">
        <legend>Your birth month and year</legend>
        <div className="community-join__pickers">
          <Select
            label="Birth month"
            hideLabel
            value={month}
            options={[
              { value: "", label: "Month" },
              ...MONTHS.map((label, i) => ({ value: String(i + 1), label })),
            ]}
            onChange={setMonth}
          />
          <Select
            label="Birth year"
            hideLabel
            value={year}
            options={[{ value: "", label: "Year" }, ...years.map((y) => ({ value: y, label: y }))]}
            onChange={setYear}
          />
        </div>
        <p className="muted">
          Asked once, and never the day. Community is for people 13 and older.
        </p>
      </fieldset>
      <p className="notice-box" role="note">
        You&apos;ll be listed in the Community directory, so people can find you. Choose Appear
        offline at any time to not be shown.
      </p>
      <WhatPeopleSee draft={draft} onChange={setDraft} disabled={busy} />
      <div className="settings-section__actions">
        <Button onClick={onTerms}>Read the Community terms</Button>
      </div>
      <Checkbox label="I agree to the Community terms" checked={agreed} onChange={setAgreed} />
      <div className="settings-section__actions">
        <Button type="submit" variant="primary" disabled={!ready || busy}>
          Join Community
        </Button>
      </div>
    </form>
  );
}

/**
 * **Your profile**: what people see, with Save. It starts from the profile as kept, follows a
 * change made in another window (when you have not changed anything here yet or have saved), and
 * Save stays off until something is different.
 */
function YourProfile({
  saved,
  hiddenParts,
  busy,
  onSave,
}: {
  /** The profile as kept. */
  saved: ProfileDraft;
  /** Parts 8 West hid. */
  hiddenParts: readonly string[];
  busy: boolean;
  onSave: (profile: ProfileDraft) => void;
}) {
  const [draft, setDraft] = useState(saved);
  // The kept profile changed (saved here, or in another window): start again from it.
  const [seen, setSeen] = useState(saved);
  if (!sameProfile(seen, saved)) {
    setSeen(saved);
    setDraft(saved);
  }
  return (
    <section className="community-profile" aria-labelledby="community-profile">
      <h3 id="community-profile">Your profile</h3>
      <WhatPeopleSee
        draft={draft}
        onChange={setDraft}
        disabled={busy}
        hiddenParts={hiddenParts}
        level={4}
      />
      <div className="settings-section__actions">
        <Button
          variant="primary"
          disabled={busy || sameProfile(draft, saved)}
          onClick={() => onSave(draft)}
        >
          Save
        </Button>
      </div>
    </section>
  );
}

/**
 * **Appear offline**: you leave the directory, New this week, and the leaderboard. People you talk
 * with see you as Offline, and your messages keep working (ADR-163 §5).
 */
function AppearOffline({
  member,
  busy,
  onChange,
}: {
  member: MemberView;
  busy: boolean;
  onChange: (offline: boolean) => void;
}) {
  return (
    <div className="community-offline">
      <Checkbox
        label="Appear offline"
        checked={member.appearOffline}
        disabled={busy}
        onChange={onChange}
      />
      <p className="muted">
        You leave the directory, New this week, and the leaderboard. People you talk with see you as
        Offline. Your messages keep working.
      </p>
      {member.appearOffline && <p>You appear offline in Community.</p>}
    </div>
  );
}

/**
 * Settings → Community (Phase 24, ADR-162, ADR-170): your 8 West account in Plenipo. Sign in with
 * a code on the account site (Plenipo never sees your password), choose your name in Community,
 * and sign out or leave. Nothing is sent to 8 West until you turn Community on in Settings →
 * Switches, or press the button here. What 8 West and other people send is shown as plain text.
 */
export function CommunitySettings({ go }: { go: Go }) {
  const { view, error: loadError, reload, show } = useCommunity();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [leaving, setLeaving] = useState(false);
  if (!view) {
    return loadError ? (
      <ErrorState title="Couldn't load Community" message={loadError} onRetry={reload} />
    ) : (
      <LoadingState label="Loading Community" />
    );
  }
  const run = (work: () => Promise<CommunityView>) => {
    setBusy(true);
    setError(null);
    work()
      .then(show)
      .catch((reason: unknown) => setError(toCommandError(reason).message))
      .finally(() => setBusy(false));
  };
  const open = (page: "signIn" | "terms") => {
    setError(null);
    openCommunityPage(page).catch((reason: unknown) => setError(toCommandError(reason).message));
  };
  const account = oneLine(view.accountName ?? "");
  const member = view.member;
  // The problem is shown once: in its notice when it is Community's own, else below.
  const shown = view.stage === "unreachable";
  const problems = [...new Set([error, shown ? null : view.problem])].filter(
    (p): p is string => p !== null && p !== "",
  );
  return (
    <div className="community">
      {view.stage === "off" && (
        <>
          <p>Community is off.</p>
          <div className="settings-section__actions">
            <Button
              variant="primary"
              disabled={busy}
              onClick={() => run(() => setCommunitySwitch(true))}
            >
              Turn on Community
            </Button>
          </div>
        </>
      )}
      {view.stage === "comingSoon" && (
        <>
          <p className="notice-box" role="note">
            <strong>Coming soon.</strong> 8 West hasn&apos;t opened Community yet. Nothing about you
            was sent.
          </p>
          <div className="settings-section__actions">
            <Button disabled={busy} onClick={() => run(checkCommunityAgain)}>
              Check again
            </Button>
          </div>
        </>
      )}
      {view.stage === "updateNeeded" && (
        <>
          <p className="notice-box" role="note">
            <strong>Update Plenipo to use Community.</strong> This version can&apos;t use it.
          </p>
          <div className="settings-section__actions">
            <Button onClick={() => go({ view: "settings", id: "updates" })}>Go to Updates</Button>
          </div>
        </>
      )}
      {view.stage === "unreachable" && (
        <>
          <p className="notice-box" role="note">
            {view.problem ?? NOT_REACHED}
          </p>
          <div className="settings-section__actions">
            <Button disabled={busy} onClick={() => run(checkCommunityAgain)}>
              Check again
            </Button>
          </div>
        </>
      )}
      {view.stage === "signingIn" && (
        <section aria-labelledby="community-sign-in">
          <h3 id="community-sign-in">Sign in to your 8 West account</h3>
          <p>Enter this code:</p>
          <p>
            <code className="community-code">{view.code ?? ""}</code>
          </p>
          <p>
            on the account site, and press Allow. Plenipo never sees your password. The code works
            once, for 10 minutes.
          </p>
          <div className="settings-section__actions">
            <Button variant="primary" icon="link" onClick={() => open("signIn")}>
              Open the sign-in page
            </Button>
            <Button disabled={busy} onClick={() => run(cancelCommunitySignIn)}>
              Cancel
            </Button>
          </div>
        </section>
      )}
      {view.stage === "joining" && (
        <>
          <p>
            Signed in as <strong>{account}</strong>.
          </p>
          <JoinForm
            terms={view.terms}
            profile={view.profile}
            busy={busy}
            onJoin={(name, month, year, terms, profile) =>
              run(() => joinCommunity(name, month, year, terms, profile))
            }
            onTerms={() => open("terms")}
          />
        </>
      )}
      {view.stage === "signedIn" && (
        <>
          <p>
            Signed in as <strong>{account}</strong>.
          </p>
          {member && (
            <>
              <p>
                Your name in Community: <strong>@{oneLine(member.name)}</strong>
              </p>
              {member.standing === "paused" && (
                <p className="notice-box" role="note">
                  8 West paused your Community
                  {member.pausedUntil != null ? ` until ${onDate(member.pausedUntil)}` : ""}.
                </p>
              )}
              {member.standing === "ended" && (
                <p className="notice-box" role="note">
                  8 West ended your Community.
                </p>
              )}
              <YourProfile
                saved={view.profile}
                hiddenParts={member.hiddenParts}
                busy={busy}
                onSave={(profile) => run(() => saveCommunityProfile(profile))}
              />
              <AppearOffline
                member={member}
                busy={busy}
                onChange={(offline) => run(() => setCommunityAppearOffline(offline))}
              />
            </>
          )}
          <BlockedPeople />
          <div className="settings-section__actions">
            <Button disabled={busy} onClick={() => run(signOutOfCommunity)}>
              Sign out of your account
            </Button>
            <Button variant="danger" disabled={busy} onClick={() => setLeaving(true)}>
              Leave Community
            </Button>
          </div>
        </>
      )}
      {view.stage === "signedOut" && (
        <>
          <p>{sentenceStart(systemWords().thisComputer)} is signed out of your 8 West account.</p>
          <div className="settings-section__actions">
            <Button variant="primary" disabled={busy} onClick={() => run(checkCommunityAgain)}>
              Sign in
            </Button>
          </div>
        </>
      )}
      {view.stage === "closed" && (
        <p className="notice-box" role="note">
          <strong>Community is closed for now.</strong> 8 West closed it for a while. Everything on{" "}
          {systemWords().thisComputer} is kept, and Plenipo checks again every hour.
        </p>
      )}
      {problems.map((problem) => (
        <p key={problem} className="form-error" role="alert">
          {problem}
        </p>
      ))}
      {view.stage !== "signingIn" && view.stage !== "joining" && <DeleteMyData />}
      {leaving && (
        <LeaveCommunityDialog
          onCancel={() => setLeaving(false)}
          onLeft={(next) => {
            setLeaving(false);
            setError(null);
            show(next);
          }}
        />
      )}
    </div>
  );
}
