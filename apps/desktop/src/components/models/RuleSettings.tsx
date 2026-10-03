/**
 * Settings → AI models → More → Agents with their own rule (ADR-041): the organization's,
 * departments', and roles' are in Who uses what (Phase 25, item 2.6).
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
    <section aria-labelledby="agent-rules-title">
      <h4 id="agent-rules-title">Agents with their own rule</h4>
      <p className="muted">
        An agent&apos;s own rule comes before its role&apos;s. To give one agent a rule of its own,
        select it on the Organization page and open its AI model tab.
      </p>
      {rows.length === 0 ? (
        <p className="empty">No agent has a rule of its own.</p>
      ) : (
        <table className="table models__agent-rules">
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
                        onApply={onApply}
                      />
                    </td>
                  </tr>
                )}
              </Fragment>
            ))}
          </tbody>
        </table>
      )}
      <Refusal error={error} />
    </section>
  );
}
