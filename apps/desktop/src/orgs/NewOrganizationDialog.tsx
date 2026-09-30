import { useState, type FormEvent } from "react";
import type { OrgListing, OrgStart, OrgSummary } from "@plenipo/types";

import { createOrganization, toCommandError } from "../api/commands";
import { Field, Footer, FormError } from "../components/org/OrgDialogs";
import { useSubmit } from "../components/org/dialogHelpers";
import { Modal } from "../components/org/Modal";

type How = OrgStart["kind"];

/**
 * New organization (Phase 21, ADR-094 §15): a name, and how to start — a template (coming
 * later), a copy of one of your organizations' setup, or from scratch.
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
        onCreated(await createOrganization(name.trim(), start()));
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
            <Field label="Template">
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
        <FormError error={error} />
        <Footer
          pending={pending}
          label="Create and open"
          disabled={name.trim() === ""}
          onCancel={onClose}
        />
      </form>
    </Modal>
  );
}
