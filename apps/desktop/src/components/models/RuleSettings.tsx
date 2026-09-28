/**
 * Settings → AI models → Model and effort rules (ADR-041): the whole organization's rule, each
 * department's, and the agents with their own. Roles keep their model choices (RoleChoices).
 */
import { Fragment, useState } from "react";
import type { ModelRule, RoutingSnapshot, RuleTarget } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { setModelRule } from "../../api/commands";
import { emptyRule, isEmptyRule, ruleSummary } from "../../routing/format";
import { useChange, type Apply } from "../../routing/useChange";
import { RuleEditor } from "./RuleEditor";
import { Refusal } from "./shared";

interface Row {
  key: string;
  target: RuleTarget;
  name: string;
  sub: string;
  rule: ModelRule;
  /** A department's or an agent's rule can be removed; the organization's is only emptied. */
  removable: boolean;
}

export function RuleSettings({ snapshot, onApply }: { snapshot: RoutingSnapshot; onApply: Apply }) {
  const [editing, setEditing] = useState<string | null>(null);
  const { pending, error, run } = useChange(onApply);
  const rows: Row[] = [
    {
      key: "organization",
      target: { layer: "organization" },
      name: "The whole organization",
      sub: "Every agent, unless a closer rule says otherwise",
      rule: snapshot.organization,
      removable: false,
    },
    ...snapshot.departments.map((d): Row => ({
      key: `department:${d.departmentId}`,
      target: { layer: "department", id: d.departmentId },
      name: d.name,
      sub: "Department: every agent in it",
      rule: d.rule,
      removable: true,
    })),
    ...snapshot.agents.map((a): Row => ({
      key: `agent:${a.positionId}`,
      target: { layer: "agent", id: a.positionId },
      name: a.title,
      sub: "Agent: its own rule",
      rule: a.rule,
      removable: true,
    })),
  ];
  const save = async (target: RuleTarget, rule: ModelRule) => {
    if (await run(() => setModelRule(target, rule))) setEditing(null);
  };
  return (
    <section aria-labelledby="rules-title">
      <h3 id="rules-title">Model and effort rules</h3>
      <p className="muted">
        Each agent gets its model and effort from the closest rule: its own, then its role&apos;s
        (below), then its department&apos;s, then the organization&apos;s. AI companies you never
        want are never used, wherever you set them. A change reaches open conversations with their
        next task, and changing only the effort never hires a new agent. To give one agent a rule of
        its own, select it on the Organization page and open its AI model tab.
      </p>
      <table className="table models__rules">
        <thead>
          <tr>
            <th scope="col">Rule for</th>
            <th scope="col">What it sets</th>
            <th scope="col">
              <span className="visually-hidden">Change</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <Fragment key={r.key}>
              <tr aria-current={editing === r.key}>
                <th scope="row">
                  {r.name}
                  <span className="table__sub">{r.sub}</span>
                </th>
                <td>{isEmptyRule(r.rule) ? "Nothing yet" : ruleSummary(snapshot, r.rule)}</td>
                <td>
                  <Button
                    variant="quiet"
                    size="sm"
                    aria-expanded={editing === r.key}
                    aria-label={`Change the rule for ${r.name}`}
                    onClick={() => setEditing(editing === r.key ? null : r.key)}
                  >
                    {editing === r.key ? "Close" : "Change"}
                  </Button>
                </td>
              </tr>
              {editing === r.key && (
                <tr>
                  <td colSpan={3}>
                    <RuleEditor
                      snapshot={snapshot}
                      rule={r.rule}
                      label={`Rule for ${r.name}`}
                      empty="None listed here: a closer rule's list, or your whole list, decides."
                      pending={pending}
                      onSave={(rule) => void save(r.target, rule)}
                      onRemove={r.removable ? () => void save(r.target, emptyRule()) : undefined}
                      onCancel={() => setEditing(null)}
                    />
                  </td>
                </tr>
              )}
            </Fragment>
          ))}
        </tbody>
      </table>
      <Refusal error={error} />
    </section>
  );
}
