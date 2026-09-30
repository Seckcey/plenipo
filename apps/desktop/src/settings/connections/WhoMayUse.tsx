import { useState } from "react";
import type { Access, AccessLevel, ConnectionsPage, PersonOption, Who } from "@plenipo/types";
import { Button, Segmented, Select } from "@plenipo/ui";

import { setConnectionAccess } from "../../api/commands";
import { Refusal } from "../../components/models/shared";
import { useRun } from "../../guard/useRun";
import { ACCESS_LABEL } from "./words";

const LEVELS: readonly AccessLevel[] = ["readOnly", "readWrite"];

const key = (who: Who) => `${who.kind}:${who.id}`;

/**
 * **Who may use it** (ADR-062 §3): roles and agents, each at Read only or Read and write. Nobody
 * to start; each one added starts at Read only. An agent's own line wins over its role's.
 */
export function WhoMayUse({
  id,
  access,
  people,
  onApply,
  onSave,
  levelWords,
  headingLevel = 4,
}: {
  id: string;
  access: Access[];
  people: PersonOption[];
  onApply: (page: ConnectionsPage) => void;
  /** How the list is saved (an add-on's, ADR-066 §3); a connection's by default. */
  onSave?: (next: Access[]) => Promise<ConnectionsPage>;
  /** What each level means here, under the list. */
  levelWords?: string;
  /** Its heading's level: 4 on a connection's card, 5 inside an add-on's card. */
  headingLevel?: 4 | 5;
}) {
  const { pending, error, run } = useRun(onApply);
  const [adding, setAdding] = useState("");
  const Heading = headingLevel === 5 ? "h5" : "h4";
  const listed = new Set(access.map((a) => key(a.who)));
  const nameOf = (who: Who) => {
    const p = people.find((x) => x.kind === who.kind && x.id === who.id);
    if (!p) return who.kind === "role" ? "A removed role" : "A removed agent";
    if (who.kind === "role") return `${p.name} (every agent in this role)`;
    return p.archived ? `${p.name} (archived)` : p.name;
  };
  const choices = people
    .filter((p) => !p.archived && !listed.has(`${p.kind}:${p.id}`))
    .map((p) => ({
      value: `${p.kind}:${p.id}`,
      label: p.kind === "agent" && p.role ? `${p.name} (${p.role})` : p.name,
      group: p.kind === "role" ? "Roles" : "Agents",
    }));
  const save = (next: Access[]) =>
    run(() => (onSave ? onSave(next) : setConnectionAccess(id, next)));
  const add = () => {
    const [kind, ...rest] = adding.split(":");
    const who: Who | null =
      kind === "role" || kind === "agent" ? { kind, id: rest.join(":") } : null;
    if (!who) return;
    void save([...access, { who, level: "readOnly" }]).then((ok) => {
      if (ok) setAdding("");
    });
  };
  return (
    <section className="connection__section" aria-labelledby={`${id}-who`}>
      <Heading id={`${id}-who`}>Who may use it</Heading>
      {access.length === 0 ? (
        <p className="empty">
          Nobody yet, so no worker can use it. Add a role or an agent; each starts at Read only.
        </p>
      ) : (
        <ul className="connection-who">
          {access.map((a) => (
            <li key={key(a.who)} className="connection-who__line">
              <span>{nameOf(a.who)}</span>
              <Segmented<AccessLevel>
                label={`${nameOf(a.who)}: how much`}
                value={a.level}
                options={LEVELS.map((l) => ({ value: l, label: ACCESS_LABEL[l] }))}
                onChange={(level) => {
                  if (level !== a.level && !pending) {
                    void save(access.map((x) => (key(x.who) === key(a.who) ? { ...x, level } : x)));
                  }
                }}
              />
              <Button
                variant="quiet"
                size="sm"
                aria-label={`Take ${nameOf(a.who)} off the list`}
                disabled={pending}
                onClick={() => void save(access.filter((x) => key(x.who) !== key(a.who)))}
              >
                Remove
              </Button>
            </li>
          ))}
        </ul>
      )}
      <p className="muted">
        {levelWords ? `${levelWords} ` : ""}An agent&apos;s own line wins over its role&apos;s. A
        project&apos;s or department&apos;s limit can still narrow it.
      </p>
      {choices.length > 0 && (
        <div className="actions">
          <Select
            label="Add a role or an agent"
            value={adding}
            options={[{ value: "", label: "Choose a role or an agent" }, ...choices]}
            onChange={setAdding}
          />
          <Button variant="secondary" size="sm" disabled={pending || adding === ""} onClick={add}>
            Add
          </Button>
        </div>
      )}
      <Refusal error={error} />
    </section>
  );
}
