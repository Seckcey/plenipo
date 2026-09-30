import { useState, type FormEvent } from "react";
import type { AgentRuntimeInfo, AiToolsPage, AiToolState } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { removePaidKey, savePaidKey } from "../../api/commands";
import { useRun } from "../../guard/useRun";
import { when } from "../../pages/words";
import { Refusal } from "../models/shared";
import type { Go } from "../views";

/** The longest name and key the desktop accepts (as its commands check them). */
const MAX_NAME = 60;
const MAX_KEY = 400;

/**
 * A paid AI tool's key (Phase 16 Wave 3, ADR-085): the name you gave it, Replace and Remove, or
 * the form to add one. The key is typed only here and kept in the Vault; Plenipo checks it with
 * the AI company before keeping it, and never shows it again. Paid work stays within your
 * spending caps.
 */
export function PaidKey({
  info,
  tool,
  onApply,
  go,
}: {
  info: AgentRuntimeInfo;
  tool: AiToolState;
  onApply: (page: AiToolsPage) => void;
  /** Opens Settings → Switches or Spending caps; absent: the words say where. */
  go?: Go | undefined;
}) {
  const label = info.label;
  const saved = tool.paidKey;
  const [editing, setEditing] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [name, setName] = useState(saved?.name ?? `${label} key`);
  const [key, setKey] = useState("");
  const { pending, error, run } = useRun<AiToolsPage>(onApply);
  const blocked = tool.paidBlocked;

  const submit = (e: FormEvent) => {
    e.preventDefault();
    void run(() => savePaidKey(info.id, name.trim(), key)).then((ok) => {
      // The key leaves this screen either way: typed again if the check refused it.
      setKey("");
      if (ok) setEditing(false);
    });
  };
  const remove = () =>
    void run(() => removePaidKey(info.id)).then((ok) => {
      if (ok) setConfirming(false);
    });

  const open = (id: "switches" | "spending", words: string) =>
    go ? (
      <Button size="sm" variant="quiet" onClick={() => go({ view: "settings", id })}>
        {words}
      </Button>
    ) : null;

  return (
    <div className="ai-tool__block paid-key">
      <p>
        Paid per use with your key, within your spending caps. A worker on {label} answers in text
        only.
      </p>
      {blocked && (
        <div className="notice-box" role="note">
          <p>{blocked}</p>
          <div className="ai-tool__buttons">
            {open("switches", "Switches")}
            {open("spending", "Spending caps")}
          </div>
        </div>
      )}
      {saved && !editing && (
        <>
          <p>
            Key saved: <strong>{saved.name}</strong>
            <span className="muted"> · saved {when(saved.updatedAt)}</span>
          </p>
          {confirming ? (
            <div className="ai-tool__buttons" role="group" aria-label={`Remove ${saved.name}?`}>
              <span>
                Remove {saved.name}? Paid work on {label} stops until you add a key again.
              </span>
              <Button size="sm" variant="danger" disabled={pending} onClick={remove}>
                Remove key
              </Button>
              <Button size="sm" variant="quiet" onClick={() => setConfirming(false)}>
                Keep it
              </Button>
            </div>
          ) : (
            <div className="ai-tool__buttons">
              <Button
                size="sm"
                disabled={pending || blocked !== null}
                aria-label={`Replace ${label}'s key`}
                onClick={() => {
                  setName(saved.name);
                  setEditing(true);
                }}
              >
                Replace key
              </Button>
              <Button
                size="sm"
                variant="quiet"
                disabled={pending}
                aria-label={`Remove ${label}'s key`}
                onClick={() => setConfirming(true)}
              >
                Remove key
              </Button>
            </div>
          )}
        </>
      )}
      {(!saved || editing) && (
        <form
          className="paid-key__form"
          aria-label={saved ? `Replace ${label}'s key` : `Add a key for ${label}`}
          onSubmit={submit}
        >
          <label className="field">
            <span>Name for the key</span>
            <input
              value={name}
              maxLength={MAX_NAME}
              required
              disabled={blocked !== null}
              onChange={(e) => setName(e.target.value)}
            />
          </label>
          <label className="field">
            <span>{label} key</span>
            <input
              type="password"
              autoComplete="off"
              spellCheck={false}
              value={key}
              maxLength={MAX_KEY}
              required
              disabled={blocked !== null}
              onChange={(e) => setKey(e.target.value)}
            />
          </label>
          <p className="muted">
            Plenipo checks the key with {label}, then keeps it in the Vault. It is never shown again
            and goes only to {label}.
          </p>
          <div className="ai-tool__buttons">
            <Button
              type="submit"
              size="sm"
              variant="primary"
              disabled={pending || blocked !== null || name.trim() === "" || key === ""}
            >
              {pending ? "Checking…" : "Save and check"}
            </Button>
            {editing && (
              <Button
                size="sm"
                variant="quiet"
                onClick={() => {
                  setKey("");
                  setEditing(false);
                }}
              >
                Cancel
              </Button>
            )}
          </div>
        </form>
      )}
      <Refusal error={error} />
    </div>
  );
}
