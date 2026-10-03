import { useState, type FormEvent } from "react";
import type { OrgListing } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import {
  applyOrganizationTemplate,
  saveOrganizationTemplate,
  toCommandError,
} from "../api/commands";
import { Field, FormError } from "../components/org/OrgDialogs";
import { useSubmit } from "../components/org/dialogHelpers";
import { useOrganization } from "../org/useOrganization";

/**
 * Settings → Organization → Templates (Phase 25, item 2.8): add a template's departments to this
 * organization (each with its manager and on-call team; more than one department is part of
 * Pro), and save this organization's setup as a template for new organizations (no work, keys,
 * connections, or files).
 */
export function TemplatesSetting({
  onFree,
  onSaved,
}: {
  onFree: boolean;
  onSaved: (listing: OrgListing) => void;
}) {
  const org = useOrganization();
  const templates = org.snapshot?.templates?.organizations ?? [];
  const [template, setTemplate] = useState(templates[0]?.id ?? "");
  const chosen = templates.find((t) => t.id === template) ?? templates[0];
  const locked = !!chosen && chosen.pro && onFree;
  const adding = useSubmit();
  const saving = useSubmit();
  const [name, setName] = useState("");
  const [saved, setSaved] = useState<string | null>(null);

  const add = () =>
    void adding.run(async () => {
      if (!chosen) return null;
      try {
        org.apply(await applyOrganizationTemplate(chosen.id));
        return null;
      } catch (reason) {
        return toCommandError(reason).message;
      }
    });
  const save = (e: FormEvent) => {
    e.preventDefault();
    void saving.run(async () => {
      try {
        onSaved(await saveOrganizationTemplate(name.trim()));
        setSaved(name.trim());
        setName("");
        return null;
      } catch (reason) {
        return toCommandError(reason).message;
      }
    });
  };

  return (
    <section className="templates-setting" aria-labelledby="templates-title">
      <h3 id="templates-title">Templates</h3>
      {templates.length > 0 && chosen && (
        <div className="templates-setting__add">
          <Field label="Add a template's departments" hint={chosen.description}>
            <select value={chosen.id} onChange={(e) => setTemplate(e.target.value)}>
              {templates.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name}
                  {t.pro && onFree ? " (part of Pro)" : ""}
                </option>
              ))}
            </select>
          </Field>
          <ul className="templates-setting__adds" aria-label={`What ${chosen.name} adds`}>
            {chosen.adds.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
          <p className="muted">
            {locked
              ? "It adds more than one department: part of Plenipo Pro. Free keeps one department."
              : "A department you already have is kept as it is; only the missing ones are added."}
          </p>
          <div className="actions">
            <Button size="sm" disabled={adding.pending || locked} onClick={add}>
              {adding.pending ? "Adding…" : "Add its departments"}
            </Button>
          </div>
          <FormError error={adding.error} />
        </div>
      )}
      <form
        className="templates-setting__save"
        aria-label="Save this organization as a template"
        onSubmit={save}
      >
        <Field
          label="Save this organization as a template"
          hint="Its departments and positions, roles, permissions, switches, AI model choices, and titles, for a new organization to start from. Never its work, keys, connections, or files."
        >
          <input
            value={name}
            maxLength={200}
            placeholder="My agency setup"
            onChange={(e) => setName(e.target.value)}
          />
        </Field>
        <div className="actions">
          <Button type="submit" size="sm" disabled={saving.pending || name.trim() === ""}>
            Save as a template
          </Button>
          {saved && (
            <span className="muted" role="status">
              Saved &quot;{saved}&quot;: New organization → Use a template offers it.
            </span>
          )}
        </div>
        <FormError error={saving.error} />
      </form>
    </section>
  );
}
