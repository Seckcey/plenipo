import { useEffect, useRef, useState, type FormEvent } from "react";
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
 * the form to add one. The key is typed only here; Plenipo checks it with the AI company, keeps it
 * in the Vault (Windows Credential Manager, as the screen says) if it works, and never shows it
 * again. A spending limit is up to you (the owner's choice, 2026-09-30): the form and the saved
 * key say so, and no cap is needed. Overview gives each saved key a new instance (`key`), so
 * nothing typed or asked for one key stays for the next.
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
  const { pending, error, run, clear } = useRun<AiToolsPage>(onApply);
  const blocked = tool.paidBlocked;
  const keptIn = tool.keyKeptIn ?? "Windows Credential Manager";
  // The keyboard goes to the key box when Replace opens the form.
  const box = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (editing) box.current?.focus();
  }, [editing]);

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

  const cancel = () => {
    setKey("");
    clear();
    setEditing(false);
  };

  const open = (id: "switches" | "spending", words: string) =>
    go ? (
      <Button size="sm" variant="quiet" onClick={() => go({ view: "settings", id })}>
        {words}
      </Button>
    ) : null;

  return (
    <div className="ai-tool__block paid-key">
      <p>Paid per use with your key. A worker on {label} answers in text only.</p>
      {/* What is not checked yet, and where to make a key (ADR-087). */}
      {tool.paidNote && <p className="muted">{tool.paidNote}</p>}
      {blocked && (
        <div className="notice-box" role="note">
          <p>{blocked}</p>
          <div className="ai-tool__buttons">{open("switches", "Open Switches")}</div>
        </div>
      )}
      {saved && !editing && (
        <>
          <p>
            Key saved: <strong>{saved.name}</strong>
            <span className="muted"> · saved {when(saved.updatedAt)}</span>
          </p>
          <SpendingLimit go={go} />
          {confirming ? (
            <div className="ai-tool__buttons" role="group" aria-label={`Remove ${saved.name}?`}>
              <span>
                Remove {saved.name}? Paid work on {label} stops until you add a key again.
              </span>
              <Button size="sm" variant="danger" disabled={pending} onClick={remove}>
                Remove key
              </Button>
              <Button
                size="sm"
                variant="quiet"
                onClick={() => {
                  clear();
                  setConfirming(false);
                }}
              >
                Keep it
              </Button>
            </div>
          ) : (
            <div className="ai-tool__buttons">
              <Button
                size="sm"
                disabled={pending || blocked !== null}
                aria-label={`Replace key for ${label}`}
                onClick={() => {
                  clear();
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
                aria-label={`Remove key for ${label}`}
                onClick={() => {
                  clear();
                  setConfirming(true);
                }}
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
          aria-label={saved ? `Replace key for ${label}` : `Add a key for ${label}`}
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
              ref={box}
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
            Plenipo checks the key with {label}. If it works, it is kept in {keptIn} and never shown
            again. Plenipo sends it only to {label}. Never paste a key into a chat.
          </p>
          <SpendingLimit go={go} />
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
              <Button size="sm" variant="quiet" onClick={cancel}>
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

/**
 * A spending limit is the owner's choice, never needed (2026-09-30): said where a key is added
 * and beside a saved key, with the way to Spending caps.
 */
function SpendingLimit({ go }: { go?: Go | undefined }) {
  return (
    <p className="muted paid-key__limit">
      You can set a spending limit in Settings → Spending caps if you want; it is not required.
      Without one, paid work has no dollar limit, and every paid task is still priced and listed
      there.{" "}
      {go && (
        <Button size="sm" variant="quiet" onClick={() => go({ view: "settings", id: "spending" })}>
          Open Spending caps
        </Button>
      )}
    </p>
  );
}
