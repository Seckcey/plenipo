import { useEffect, useId, useState, type FormEvent } from "react";
import type { OrgFolderInfo, OrgListing, OrgStart, OrgSummary } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import {
  chooseFolder,
  createOrganization,
  suggestOrgFolder,
  toCommandError,
} from "../api/commands";
import { Field, Footer, FormError } from "../components/org/OrgDialogs";
import { useSubmit } from "../components/org/dialogHelpers";
import { Modal } from "../components/org/Modal";
import { KeepOnThisDeviceAlert } from "./OrgFolder";

/** How long typing pauses before the folder's place is looked up again. */
const LOOK_UP_AFTER_MS = 250;

type How = OrgStart["kind"];

/**
 * New organization (Phase 21, ADR-094 §15): a name, how to start — a template, a copy of one of
 * your organizations' setup, or from scratch — and its organization folder (ADR-205): in
 * Documents → Plenipo, or a folder the owner chooses.
 */
export function NewOrganizationDialog({
  listing,
  onClose,
  onCreated,
}: {
  listing: OrgListing;
  onClose: () => void;
  /** Made: Plenipo opens it (in a new window). */
  onCreated: (org: OrgSummary) => void;
}) {
  const others = listing.organizations.filter((o) => !o.archived);
  const [name, setName] = useState("");
  const [how, setHow] = useState<How>("scratch");
  const [from, setFrom] = useState(listing.current);
  const { pending, error, run } = useSubmit();
  const templates = listing.templates;
  const [template, setTemplate] = useState(templates[0]?.id ?? "");
  // The folder the owner chose (`null`: the usual place), and where the organization's folder
  // would be.
  const [chosen, setChosen] = useState<string | null>(null);
  const [folder, setFolder] = useState<OrgFolderInfo | null>(null);
  const [chooseError, setChooseError] = useState<string | null>(null);
  const folderLabel = useId();

  useEffect(() => {
    let live = true;
    const timer = setTimeout(() => {
      suggestOrgFolder(name.trim(), chosen)
        .then((info) => {
          if (live) setFolder(info);
        })
        .catch((reason: unknown) => {
          if (live) setChooseError(toCommandError(reason).message);
        });
    }, LOOK_UP_AFTER_MS);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [name, chosen]);

  const change = () => {
    setChooseError(null);
    chooseFolder()
      .then((picked) => {
        if (!picked?.path) return;
        if (picked.problem) setChooseError(picked.problem);
        else setChosen(picked.path);
      })
      .catch((reason: unknown) => setChooseError(toCommandError(reason).message));
  };

  const start = (): OrgStart =>
    how === "copy"
      ? { kind: "copy", from }
      : how === "template"
        ? { kind: "template", template }
        : { kind: "scratch" };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    void run(async () => {
      try {
        onCreated(await createOrganization(name.trim(), start(), chosen));
        return null;
      } catch (reason) {
        return toCommandError(reason).message;
      }
    });
  };

  const choice = (value: How, label: string, hint: string, disabled = false) => (
    <label className={`choice${disabled ? " choice--disabled" : ""}`}>
      <input
        type="radio"
        name="start"
        value={value}
        checked={how === value}
        disabled={disabled}
        onChange={() => setHow(value)}
      />
      <span className="choice__text">
        <span className="choice__label">{label}</span>
        <span className="muted">{hint}</span>
      </span>
    </label>
  );

  return (
    <Modal title="New organization" onClose={onClose}>
      <form className="modal__body" aria-label="New organization" onSubmit={submit}>
        <p className="muted">
          An organization of its own, for example a client&apos;s: its own org chart, projects,
          work, approvals, and secrets. Your Workforce and the AI tools are shared.
        </p>
        <Field label="Name">
          <input
            value={name}
            maxLength={200}
            required
            onChange={(e) => setName(e.target.value)}
            placeholder="Client Co"
          />
        </Field>
        <fieldset className="choices choices--stack">
          <legend>How should it start?</legend>
          {choice(
            "template",
            "Use a template",
            templates.length === 0
              ? "Templates are coming later."
              : "A ready-made setup to start from.",
            templates.length === 0,
          )}
          {how === "template" && templates.length > 0 && (
            <Field label="Template" hint={templates.find((t) => t.id === template)?.description}>
              <select value={template} onChange={(e) => setTemplate(e.target.value)}>
                {templates.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.name}
                  </option>
                ))}
              </select>
            </Field>
          )}
          {choice(
            "copy",
            "Copy from one of your organizations",
            "Its departments and positions, roles, permissions, switches, AI model choices, and titles. No one is hired, and no work, projects, servers, connections, or secrets are copied.",
          )}
          {how === "copy" && (
            <Field label="Copy from">
              <select value={from} onChange={(e) => setFrom(e.target.value)}>
                {others.map((o) => (
                  <option key={o.id} value={o.id}>
                    {o.name}
                  </option>
                ))}
              </select>
            </Field>
          )}
          {choice(
            "scratch",
            "Start from scratch",
            "Plenipo's starting settings, as a new copy has.",
          )}
        </fieldset>
        {/* Not a <label>: a click on its words must not press Change…. */}
        <div className="field" role="group" aria-labelledby={folderLabel}>
          <span id={folderLabel}>Organization folder</span>
          <div className="org-folder__where">
            <code className="org-folder__path" aria-label="Where its folder goes">
              {folder?.path ?? "…"}
            </code>
            <Button size="sm" onClick={change}>
              Change…
            </Button>
          </div>
          <small className="field__hint">
            Plenipo keeps a folder here for each department and project, with their finished files,
            and a scratch pad for each worker.
          </small>
        </div>
        <FormError error={folder?.problem ?? chooseError} />
        <KeepOnThisDeviceAlert info={folder?.problem ? null : folder} />
        <FormError error={error} />
        <Footer
          pending={pending}
          label="Create and open"
          disabled={name.trim() === "" || Boolean(folder?.problem)}
          onCancel={onClose}
        />
      </form>
    </Modal>
  );
}
