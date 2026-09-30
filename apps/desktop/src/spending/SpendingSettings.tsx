import { useState } from "react";
import type { CapCovers, CapStatus, OrgSnapshot, SpendingPage } from "@plenipo/types";
import { Button, ErrorState, HealthBar, LoadingState, Select, TextField } from "@plenipo/ui";

import { getSpending, removeSpendingCap, setSpendingCap, toCommandError } from "../api/commands";
import type { Go } from "../components/views";
import { useOrganizationNames } from "../org/useOrganizationNames";
import { useLive } from "../pages/useLive";
import { useShown } from "../upkeep/useShown";
import {
  AMOUNT_HELP,
  capLine,
  capStateWords,
  coversKey,
  dollars,
  monthName,
  parseDollars,
  recordCost,
  recordWho,
  resetDay,
  typedAmount,
  when,
} from "./words";

/** Ledger events after which the page has something new to show. */
function relevant(eventType: string): boolean {
  return eventType.startsWith("spending.") || eventType.startsWith("org.");
}

/**
 * Settings → Spending caps (Phase 16 Wave 3, ADR-085; ADR-036 §2): the monthly caps for the
 * business, each department, and single positions, what this month has used of each, and the
 * month's paid tasks. No paid AI key works without the business's cap. Money is set aside before
 * a paid task starts, so a cap is never passed: work stops a little before 100% instead.
 */
export function SpendingSettings({ go }: { go: Go }) {
  const live = useLive<SpendingPage>(
    "spending",
    () => getSpending(),
    (e) => relevant(e.eventType),
  );
  const org = useOrganizationNames().snapshot;
  const [page, setShown] = useShown(live);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  if (live.status === "loading" && !page) return <LoadingState label="Loading spending caps" />;
  if (!page) {
    return (
      <ErrorState
        title="Couldn't load your spending caps"
        message={live.error}
        onRetry={live.reload}
      />
    );
  }

  const act = async (key: string, work: () => Promise<SpendingPage>) => {
    setBusy(key);
    setError(null);
    try {
      setShown(await work());
      live.reload();
      return true;
    } catch (reason) {
      setError(toCommandError(reason).message);
      return false;
    } finally {
      setBusy(null);
    }
  };
  const set = (covers: CapCovers, micros: number) =>
    act(coversKey(covers), () => setSpendingCap(covers, micros));
  const remove = (status: CapStatus) =>
    act(coversKey(status.cap.covers), () => removeSpendingCap(status.cap.id));

  const capOf = (key: string) => page.caps.find((c) => coversKey(c.cap.covers) === key);
  const business = capOf("business");

  return (
    <div className="spending">
      <p className="notice-box" role="note">
        <strong>Paid AI keys spend real money.</strong> Before a paid task starts, Plenipo sets
        aside the most it could cost, and never starts one that could pass a cap. At 80% of a cap
        you get a warning; at the cap, paid work stops until the month starts over or you raise the
        cap. Your subscriptions are never counted here. Workers use paid keys only when{" "}
        <strong>Let workers use paid AI keys</strong> is on in Settings → Switches.{" "}
        <Button variant="quiet" size="sm" onClick={() => go({ view: "settings", id: "switches" })}>
          Open Switches
        </Button>
      </p>

      <section aria-labelledby="spending-month">
        <h3 id="spending-month">This month ({monthName(page.month)})</h3>
        <dl className="spending__month">
          <div>
            <dt>Spent</dt>
            <dd className="ui-num">{dollars(page.spentMicros)}</dd>
          </div>
          <div>
            <dt>Set aside for running tasks</dt>
            <dd className="ui-num">{dollars(page.setAsideMicros)}</dd>
          </div>
          <div>
            <dt>Not priced yet</dt>
            <dd className="ui-num">
              {page.notPriced === 1 ? "1 task" : `${page.notPriced} tasks`}
            </dd>
          </div>
          <div>
            <dt>Starts over</dt>
            <dd>{resetDay(page.resetsAt)} (Pacific time)</dd>
          </div>
        </dl>
      </section>

      <section aria-labelledby="spending-business">
        <h3 id="spending-business">The business</h3>
        {!page.hasBusinessCap && (
          <p className="form-error" role="status">
            No paid AI key works until you set the business&apos;s monthly cap.
          </p>
        )}
        <CapEditor
          key={business?.cap.id ?? "none"}
          label="The business"
          covers={{ kind: "business" }}
          status={business}
          busy={busy === "business"}
          onSet={set}
          onRemove={remove}
        />
      </section>

      <section aria-labelledby="spending-departments">
        <h3 id="spending-departments">Departments</h3>
        <p className="muted">
          A department&apos;s cap covers its positions&apos; paid tasks. Without one, the
          business&apos;s cap covers them.
        </p>
        <DepartmentCaps page={page} org={org} busy={busy} onSet={set} onRemove={remove} />
      </section>

      <section aria-labelledby="spending-positions">
        <h3 id="spending-positions">Positions</h3>
        <p className="muted">
          A position&apos;s cap covers its own paid tasks. The department&apos;s and the
          business&apos;s caps still apply: the smallest amount left decides.
        </p>
        <PositionCaps page={page} org={org} busy={busy} onSet={set} onRemove={remove} />
      </section>

      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}

      <section aria-labelledby="spending-tasks">
        <h3 id="spending-tasks">This month&apos;s paid tasks</h3>
        {page.recent.length === 0 ? (
          <p className="muted">No paid tasks this month.</p>
        ) : (
          <ul className="spending__tasks">
            {page.recent.map((r) => (
              <li key={r.id}>
                <span className="spending__task-who">{recordWho(r)}</span>
                <span className="muted">
                  {r.model}
                  {r.keyName ? ` · key: ${r.keyName}` : ""} · {when(r.createdAt)}
                </span>
                <span className="ui-num">{recordCost(r)}</span>
                {r.detail && <span className="muted">{r.detail}</span>}
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

type SetCap = (covers: CapCovers, micros: number) => Promise<boolean>;
type RemoveCap = (status: CapStatus) => Promise<boolean>;

/** A cap: its bar, where it stands, and its amount to set, change, or remove. */
function CapEditor({
  label,
  covers,
  status,
  busy,
  onSet,
  onRemove,
}: {
  label: string;
  covers: CapCovers;
  status: CapStatus | undefined;
  busy: boolean;
  onSet: SetCap;
  onRemove: RemoveCap;
}) {
  const [amount, setAmount] = useState(status ? typedAmount(status.cap.monthlyMicros) : "");
  const [wrong, setWrong] = useState(false);
  const save = async () => {
    const micros = parseDollars(amount);
    setWrong(micros === null);
    if (micros !== null) await onSet(covers, micros);
  };
  const state = status ? capStateWords(status) : null;
  const spoken = covers.kind === "business" ? "the business" : label;
  return (
    <div className="spending__cap">
      {status && state && (
        <>
          <HealthBar
            label={`${label}: ${state.word}`}
            value={status.spentMicros + status.setAsideMicros}
            max={status.cap.monthlyMicros}
            valueText={capLine(status)}
            status={state.status}
          />
          <p className="muted">
            {dollars(status.leftMicros)} left this month for new paid tasks.
            {status.stoppedWhy ? ` ${status.stoppedWhy}` : ""}
          </p>
        </>
      )}
      <div className="spending__cap-form">
        <TextField
          label={`${label}: monthly cap in dollars`}
          value={amount}
          onChange={(next) => {
            setAmount(next);
            setWrong(false);
          }}
          placeholder="50"
          hint={wrong ? AMOUNT_HELP : undefined}
        />
        <Button
          variant="primary"
          size="sm"
          disabled={busy}
          aria-label={`${status ? "Change" : "Set"} the cap for ${spoken}`}
          onClick={() => void save()}
        >
          {status ? "Change the cap" : "Set the cap"}
        </Button>
        {status && (
          <Button
            variant="quiet"
            size="sm"
            disabled={busy}
            aria-label={`Remove the cap for ${spoken}`}
            onClick={() => void onRemove(status)}
          >
            Remove the cap
          </Button>
        )}
      </div>
    </div>
  );
}

/** Each department in the organization, with its cap or a place to set one. */
function DepartmentCaps({
  page,
  org,
  busy,
  onSet,
  onRemove,
}: {
  page: SpendingPage;
  org: OrgSnapshot | null;
  busy: string | null;
  onSet: SetCap;
  onRemove: RemoveCap;
}) {
  const departments = (org?.departments ?? []).filter((d) => d.active && !d.deleted);
  const gone = page.caps.filter((c) => c.cap.covers.kind === "department" && c.gone);
  if (departments.length === 0 && gone.length === 0) {
    return <p className="muted">No departments yet. Add them on the Organization map.</p>;
  }
  return (
    <ul className="spending__list">
      {departments.map((d) => {
        const covers: CapCovers = { kind: "department", id: d.id };
        const status = page.caps.find((c) => coversKey(c.cap.covers) === coversKey(covers));
        return (
          <li key={d.id}>
            <CapEditor
              key={status?.cap.id ?? "none"}
              label={d.name}
              covers={covers}
              status={status}
              busy={busy === coversKey(covers)}
              onSet={onSet}
              onRemove={onRemove}
            />
          </li>
        );
      })}
      {gone.map((status) => (
        <li key={status.cap.id}>
          <GoneCap status={status} busy={busy} onRemove={onRemove} />
        </li>
      ))}
    </ul>
  );
}

/** The positions with a cap, and a place to add one. */
function PositionCaps({
  page,
  org,
  busy,
  onSet,
  onRemove,
}: {
  page: SpendingPage;
  org: OrgSnapshot | null;
  busy: string | null;
  onSet: SetCap;
  onRemove: RemoveCap;
}) {
  const withCaps = page.caps.filter((c) => c.cap.covers.kind === "position");
  const capped = new Set(withCaps.map((c) => coversKey(c.cap.covers)));
  const open = (org?.positions ?? []).filter(
    (p) => p.active && !p.deleted && !capped.has(`position:${p.id}`),
  );
  const [chosen, setChosen] = useState("");
  const [amount, setAmount] = useState("");
  const [wrong, setWrong] = useState(false);
  const pick = chosen && open.some((p) => p.id === chosen) ? chosen : (open[0]?.id ?? "");
  const add = async () => {
    const micros = parseDollars(amount);
    setWrong(micros === null);
    if (micros === null || !pick) return;
    if (await onSet({ kind: "position", id: pick }, micros)) setAmount("");
  };
  return (
    <>
      {withCaps.length > 0 && (
        <ul className="spending__list">
          {withCaps.map((status) => (
            <li key={status.cap.id}>
              {status.gone ? (
                <GoneCap status={status} busy={busy} onRemove={onRemove} />
              ) : (
                <CapEditor
                  label={status.label}
                  covers={status.cap.covers}
                  status={status}
                  busy={busy === coversKey(status.cap.covers)}
                  onSet={onSet}
                  onRemove={onRemove}
                />
              )}
            </li>
          ))}
        </ul>
      )}
      {open.length > 0 ? (
        <div className="spending__cap-form">
          <Select
            label="Add a cap for one position"
            value={pick}
            options={open.map((p) => ({ value: p.id, label: p.title }))}
            onChange={setChosen}
          />
          <TextField
            label="Its monthly cap in dollars"
            value={amount}
            onChange={(next) => {
              setAmount(next);
              setWrong(false);
            }}
            placeholder="10"
            hint={wrong ? AMOUNT_HELP : undefined}
          />
          <Button
            variant="primary"
            size="sm"
            disabled={busy === `position:${pick}`}
            onClick={() => void add()}
          >
            Add the cap
          </Button>
        </div>
      ) : (
        withCaps.length === 0 && <p className="muted">No positions yet.</p>
      )}
    </>
  );
}

/** A cap whose department or position is no longer in the organization. */
function GoneCap({
  status,
  busy,
  onRemove,
}: {
  status: CapStatus;
  busy: string | null;
  onRemove: RemoveCap;
}) {
  return (
    <div className="spending__cap">
      <p>
        {status.label}: {dollars(status.cap.monthlyMicros)} a month.{" "}
        <span className="muted">It no longer covers any work.</span>
      </p>
      <Button
        variant="quiet"
        size="sm"
        disabled={busy === coversKey(status.cap.covers)}
        aria-label={`Remove the cap for ${status.label}`}
        onClick={() => void onRemove(status)}
      >
        Remove the cap
      </Button>
    </div>
  );
}
