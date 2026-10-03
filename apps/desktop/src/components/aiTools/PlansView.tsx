/**
 * Your plans (Phase 25, item 4.6; ADR-258): every AI plan, how its use compares with a fair share
 * of its window so far, when it starts again, and paid spending this month, in one place on the
 * AI tools page. An AI tool that reports nothing of its plan can be paced against a weekly budget
 * of tokens the owner sets, marked "estimated".
 */
import type { SpendingPage, ToolPaces, WindowPace } from "@plenipo/types";
import { Button, StatusPill, type Status } from "@plenipo/ui";
import { useCallback, useEffect, useState } from "react";

import { getPlanPaces, getSpending, setPlanBudget, toCommandError } from "../../api/commands";
import { dollars } from "../../spending/words";
import { paceWords, planWindowName, resetWhen } from "./words";

/** What the pace means for the work, in a few words. */
function paceMeans(w: WindowPace): string | null {
  if (w.pace === "ahead") return "Work steps down early to make it last.";
  if (w.pace === "behind") return "Room to spare: the best model is used freely.";
  return null;
}

const PACE_STATUS: Record<WindowPace["pace"], Status> = {
  ahead: "warn",
  onPace: "ok",
  behind: "ok",
  unknown: "pending",
};

const PACE_LABEL: Record<WindowPace["pace"], string> = {
  ahead: "Ahead of pace",
  onPace: "On pace",
  behind: "Behind pace",
  unknown: "Not paced",
};

/** A window's name: "Week (Opus)", "5-hour", or "Plan". */
function windowName(w: WindowPace): string {
  const name = planWindowName(w.minutes, w.models) ?? "Plan";
  return w.estimated ? `${name}, estimated` : name;
}

/** The owner's weekly budget of tokens for an AI tool that reports nothing. */
function BudgetForm({ tool, onSaved }: { tool: ToolPaces; onSaved: () => Promise<void> }) {
  const [value, setValue] = useState(tool.weeklyBudget ? String(tool.weeklyBudget) : "");
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const save = async (tokens: number | null) => {
    setPending(true);
    setError(null);
    try {
      await setPlanBudget(tool.runtimeId, tokens);
      await onSaved();
    } catch (reason) {
      setError(toCommandError(reason).message);
    } finally {
      setPending(false);
    }
  };
  const tokens = Number(value.replace(/[\s,]/g, ""));
  const valid = Number.isInteger(tokens) && tokens > 0;
  return (
    <form
      className="plans__budget"
      onSubmit={(e) => {
        e.preventDefault();
        if (valid) void save(tokens);
      }}
    >
      <label className="field field--inline">
        <span>Weekly budget for {tool.label} (tokens)</span>
        <input
          inputMode="numeric"
          value={value}
          placeholder="2,000,000"
          disabled={pending}
          onChange={(e) => setValue(e.target.value)}
        />
      </label>
      <Button size="sm" type="submit" disabled={pending || !valid}>
        Save
      </Button>
      {tool.weeklyBudget !== null && (
        <Button size="sm" variant="quiet" disabled={pending} onClick={() => void save(null)}>
          Remove
        </Button>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}

/** Every AI plan and its pace, and paid spending this month. */
export function PlansView() {
  const [tools, setTools] = useState<ToolPaces[] | null>(null);
  const [spending, setSpending] = useState<SpendingPage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => {
    try {
      setTools(await getPlanPaces());
      setError(null);
    } catch (reason) {
      setError(toCommandError(reason).message);
    }
    // Spending is shown when it can be read; the plans don't depend on it.
    await getSpending().then(setSpending, () => setSpending(null));
  }, []);
  useEffect(() => {
    let live = true;
    Promise.resolve()
      .then(() => Promise.all([getPlanPaces(), getSpending().catch(() => null)]))
      .then(
        ([paces, page]) => {
          if (!live) return;
          setTools(paces);
          setSpending(page);
        },
        (reason: unknown) => {
          if (live) setError(toCommandError(reason).message);
        },
      );
    return () => {
      live = false;
    };
  }, []);

  const business = spending?.caps.find((c) => c.cap.covers.kind === "business");
  return (
    <section className="plans" aria-labelledby="plans-title">
      <div className="section-header">
        <h2 id="plans-title">Your plans</h2>
      </div>
      <p className="muted">
        Plenipo spreads each plan&apos;s use over its window, so it lasts until it starts again:
        ahead of pace, work steps down early; behind pace, the best model is used freely. Night
        hours count for less (Settings → Switches).
      </p>
      {error && (
        <p className="form-error" role="alert">
          Couldn&apos;t read your plans: {error}
        </p>
      )}
      {tools === null && !error && <p className="muted">Reading your plans…</p>}
      {tools && tools.length === 0 && (
        <p className="muted">No AI tool with a subscription is ready yet.</p>
      )}
      {tools && tools.length > 0 && (
        <ul className="plans__list" aria-label="Your plans">
          {tools.map((t) => (
            <li key={t.runtimeId} className="plans__tool" aria-label={t.label}>
              <h3>{t.label}</h3>
              {t.windows.length === 0 ? (
                <>
                  <p className="muted">
                    {t.label} doesn&apos;t report how much of its plan is used. Give it a weekly
                    budget of tokens, and Plenipo paces it with its own counts (estimated).
                  </p>
                  <BudgetForm tool={t} onSaved={load} />
                </>
              ) : (
                <>
                  <table className="plans__table">
                    <thead>
                      <tr>
                        <th scope="col">Window</th>
                        <th scope="col">Used</th>
                        <th scope="col">Pace</th>
                        <th scope="col">Starts again</th>
                      </tr>
                    </thead>
                    <tbody>
                      {t.windows.map((w, i) => (
                        <tr key={i}>
                          <th scope="row">{windowName(w)}</th>
                          <td>{paceWords(w)}</td>
                          <td>
                            <StatusPill status={PACE_STATUS[w.pace]} label={PACE_LABEL[w.pace]} />
                            {paceMeans(w) && <span className="muted"> {paceMeans(w)}</span>}
                          </td>
                          <td>{w.resetsAt === null ? "—" : resetWhen(w.resetsAt)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  {t.weeklyBudget !== null && <BudgetForm tool={t} onSaved={load} />}
                </>
              )}
            </li>
          ))}
        </ul>
      )}
      {spending && (
        <p className="plans__spending">
          Paid AI this month: {dollars(spending.spentMicros)}
          {business ? ` of your ${dollars(business.cap.monthlyMicros)} cap` : ""}
          {spending.setAsideMicros > 0 ? ` (${dollars(spending.setAsideMicros)} set aside)` : ""}.
        </p>
      )}
    </section>
  );
}
