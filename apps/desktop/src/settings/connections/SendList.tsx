import { useState } from "react";
import type { ConnectionsPage } from "@plenipo/types";
import { Button, TextField } from "@plenipo/ui";

import { setConnectionSendList } from "../../api/commands";
import { Refusal } from "../../components/models/shared";
import type { Go } from "../../components/views";
import { useRun } from "../../guard/useRun";

/**
 * **Send without asking to** (ADR-062 §5): addresses, `@domains`, and channels a worker may send
 * to without asking you — only while the switch "Sending forms and messages (without asking)" is
 * on, and only when every recipient is on this list.
 */
export function SendList({
  id,
  list,
  switchOn,
  go,
  onApply,
}: {
  id: string;
  list: string[];
  switchOn: boolean;
  go: Go;
  onApply: (page: ConnectionsPage) => void;
}) {
  const { pending, error, run } = useRun(onApply);
  const [entry, setEntry] = useState("");
  const save = (next: string[]) => run(() => setConnectionSendList(id, next));
  return (
    <section className="connection__section" aria-labelledby={`${id}-send`}>
      <h4 id={`${id}-send`}>Send without asking to</h4>
      <p className="notice-box" role="note">
        <strong>
          An email could trick a worker into writing to anyone on this list without asking you.
        </strong>{" "}
        Add only addresses and domains you would be happy to receive anything a worker writes.
        Everyone else, and every send with someone not on the list, still asks you. Posting in a
        Teams channel always asks you.
      </p>
      <p className="muted">
        {switchOn
          ? "Used now: the switch Sending forms and messages (without asking) is on."
          : "Not used now: the switch Sending forms and messages (without asking) is off, so every send asks you."}{" "}
        <Button variant="quiet" size="sm" onClick={() => go({ view: "settings", id: "switches" })}>
          Open Settings → Switches
        </Button>
      </p>
      {list.length === 0 ? (
        <p className="empty">Nobody on the list: every send asks you.</p>
      ) : (
        <ul className="connection-send">
          {list.map((e) => (
            <li key={e}>
              <span className="path">{e}</span>{" "}
              <Button
                variant="quiet"
                size="sm"
                aria-label={`Take ${e} off the list`}
                disabled={pending}
                onClick={() => void save(list.filter((x) => x !== e))}
              >
                Remove
              </Button>
            </li>
          ))}
        </ul>
      )}
      <form
        className="actions"
        onSubmit={(ev) => {
          ev.preventDefault();
          const next = entry.trim();
          if (!next) return;
          void save([...list, next]).then((ok) => {
            if (ok) setEntry("");
          });
        }}
      >
        <TextField
          label="An address or an @domain"
          value={entry}
          placeholder="dana@clientco.com or @clientco.com"
          onChange={setEntry}
        />
        <Button
          type="submit"
          variant="secondary"
          size="sm"
          disabled={pending || entry.trim() === ""}
        >
          Add to the list
        </Button>
      </form>
      <Refusal error={error} />
    </section>
  );
}
