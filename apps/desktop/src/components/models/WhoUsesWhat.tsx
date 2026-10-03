/**
 * Settings → AI models → **Who uses what** (Phase 25, item 2.6): one table of each role's model,
 * its backup, and its effort, with a row for the whole organization and one for each department.
 * Each row's Change opens its editor below it. The next worker of each role says which model it
 * gets and where that came from.
 */
import { Fragment, useState } from "react";
import type { ModelRule, RolePolicyView, RouteDecision, RoutingSnapshot } from "@plenipo/types";
import { Button, StatusPill } from "@plenipo/ui";

import { setModelRule } from "../../api/commands";
import {
  EFFORT_LABEL,
  choiceLabel,
  emptyRule,
  modelFromWords,
  modelLabel,
} from "../../routing/format";
import { useChange, type Apply } from "../../routing/useChange";
import { PILL_TONE } from "../tones";
import { PolicyEditor } from "./RoleChoices";
import { neverUsed } from "./neverUsed";
import { RuleEditor } from "./RuleEditor";
import { Refusal } from "./shared";

type RowKind =
  | { kind: "organization"; rule: ModelRule }
  | { kind: "department"; id: string; rule: ModelRule }
  | { kind: "role"; view: RolePolicyView };

interface Row {
  key: string;
  name: string;
  sub: string;
  row: RowKind;
}

export function WhoUsesWhat({ snapshot, onApply }: { snapshot: RoutingSnapshot; onApply: Apply }) {
  const [editing, setEditing] = useState<string | null>(null);
  const { pending, error, run } = useChange(onApply);
  const byId = new Map(snapshot.models.map((m) => [m.id, m]));
  const label = (id: string | undefined) => {
    if (id === undefined) return "—";
    const m = byId.get(id);
    return m ? modelLabel(snapshot, m) : "A removed model";
  };
  const backup = (models: string[]) =>
    models.length < 2
      ? "—"
      : `${label(models[1])}${models.length > 2 ? ` and ${models.length - 2} more` : ""}`;
  const effortWords = (e: RuleEffort) => (e ? `${EFFORT_LABEL[e]} effort` : "Its own");
  const rows: Row[] = [
    {
      key: "organization",
      name: "The whole organization",
      sub: "Every agent, unless a closer row says otherwise",
      row: { kind: "organization", rule: snapshot.organization },
    },
    ...snapshot.departments.map((d): Row => ({
      key: `department:${d.departmentId}`,
      name: d.name,
      sub: "Department: every agent in it",
      row: { kind: "department", id: d.departmentId, rule: d.rule },
    })),
    ...snapshot.roles.map((r): Row => ({
      key: `role:${r.roleId}`,
      name: r.roleName,
      sub: r.fullTime ? "Full-time" : "On call",
      row: { kind: "role", view: r },
    })),
  ];
  const saveRule = async (target: Parameters<typeof setModelRule>[0], rule: ModelRule) => {
    if (await run(() => setModelRule(target, rule))) setEditing(null);
  };
  return (
    <section aria-labelledby="who-uses-what-title" data-tour="who-uses-what">
      <h3 id="who-uses-what-title">Who uses what</h3>
      <p className="muted">
        Each worker gets the first model on the closest list that is ready: its own, then its
        role&apos;s, then its department&apos;s, then the whole organization&apos;s. The backups are
        tried in order. A change reaches open conversations with their next task, and changing only
        the effort never hires a new agent. To give one agent a rule of its own, select it on the
        Organization page and open its AI model tab.
      </p>
      <table className="table models__who models__rules models__roles">
        <thead>
          <tr>
            <th scope="col">Who</th>
            <th scope="col">Model</th>
            <th scope="col">Backup</th>
            <th scope="col">Effort</th>
            <th scope="col">Next worker gets</th>
            <th scope="col">
              <span className="visually-hidden">Change</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map(({ key, name, sub, row }) => {
            const models = row.kind === "role" ? row.view.policy.models : row.rule.models;
            const effort = row.kind === "role" ? row.view.policy.effort : row.rule.effort;
            const open = editing === key;
            return (
              <Fragment key={key}>
                <tr aria-current={open}>
                  <th scope="row">
                    {name}
                    <span className="table__sub">{sub}</span>
                  </th>
                  <td>{models.length > 0 ? label(models[0]) : "—"}</td>
                  <td>{backup(models)}</td>
                  <td>{effortWords(effort)}</td>
                  <td className="models__why">
                    {row.kind === "role" ? <NextWorker route={row.view.next} /> : "—"}
                  </td>
                  <td>
                    <Button
                      variant="quiet"
                      size="sm"
                      aria-expanded={open}
                      aria-label={
                        row.kind === "role"
                          ? `Change ${name}'s model choices`
                          : `Change the rule for ${name}`
                      }
                      onClick={() => setEditing(open ? null : key)}
                    >
                      {open ? "Close" : "Change"}
                    </Button>
                  </td>
                </tr>
                {open && (
                  <tr>
                    <td colSpan={6}>
                      {row.kind === "role" ? (
                        <PolicyEditor
                          snapshot={snapshot}
                          view={row.view}
                          onApply={onApply}
                          onDone={() => setEditing(null)}
                        />
                      ) : (
                        <RuleEditor
                          snapshot={snapshot}
                          rule={row.rule}
                          label={`Rule for ${name}`}
                          empty="None listed here: a closer list, or your whole list, decides."
                          pending={pending}
                          neverHere={row.kind === "organization"}
                          neverFrom={row.kind === "organization" ? [] : neverUsed(snapshot, {})}
                          onSave={(rule) =>
                            void saveRule(
                              row.kind === "organization"
                                ? { layer: "organization" }
                                : { layer: "department", id: row.id },
                              rule,
                            )
                          }
                          onRemove={
                            row.kind === "department"
                              ? () =>
                                  void saveRule({ layer: "department", id: row.id }, emptyRule())
                              : undefined
                          }
                          onCancel={() => setEditing(null)}
                          onApply={onApply}
                        />
                      )}
                    </td>
                  </tr>
                )}
              </Fragment>
            );
          })}
        </tbody>
      </table>
      <Refusal error={error} />
    </section>
  );
}

type RuleEffort = ModelRule["effort"];

/** A role's next worker: its model and where it came from, or none, and why. */
function NextWorker({ route }: { route: RouteDecision }) {
  const from = modelFromWords(route);
  return (
    <>
      {route.choice ? (
        <strong>{choiceLabel(route.choice)}</strong>
      ) : (
        <StatusPill status={PILL_TONE.warn} label="None right now" />
      )}
      {from && <span className="table__sub">{from}</span>}
      <span className="table__sub">{route.reason}</span>
    </>
  );
}
