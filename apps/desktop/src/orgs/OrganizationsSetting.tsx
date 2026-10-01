import { useState, type FormEvent } from "react";
import type { OrgDeletePreview, OrgSummary } from "@plenipo/types";
import { Button, StatusPill } from "@plenipo/ui";

import {
  archiveOrganization,
  bringBackOrganization,
  deleteOrganizationForGood,
  openOrganizationWindow,
  previewDeleteOrganization,
  renameOrganization,
  switchOrganization,
  toCommandError,
} from "../api/commands";
import { Field, Footer, FormError } from "../components/org/OrgDialogs";
import { useSubmit } from "../components/org/dialogHelpers";
import { Modal } from "../components/org/Modal";
import { useLicense } from "../license/useLicense";
import { useOrganization } from "../org/useOrganization";
import { NewOrganizationDialog } from "./NewOrganizationDialog";
import { useOrganizations } from "./useOrganizations";

function where(o: OrgSummary): string {
  if (o.archived) return "Archived";
  if (o.here) return "Shown in this window";
  if (o.inWindow) return "Open in another window";
  return o.working > 0 ? "Working in the background" : "Not open in a window";
}

/**
 * Settings → Organization (Phase 21, ADR-094 §15–18): this organization's name, and your other
 * organizations — switch, open in a new window, archive, bring back, and delete for good.
 */
export function OrganizationsSetting() {
  const license = useLicense().view;
  const onFree = license?.edition === "free";
  const { listing, error: listError, apply } = useOrganizations();
  // Counted from the list, which follows every change at once (ADR-119).
  const inUse = listing?.organizations.filter((o) => !o.archived).length ?? 0;
  const atPlanLimit =
    license?.edition === "pro" &&
    license.organizationsCovered !== null &&
    inUse >= license.organizationsCovered;
  const org = useOrganization();
  const [creating, setCreating] = useState(false);
  const [deleting, setDeleting] = useState<OrgDeletePreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState<string | null>(null);
  const rename = useSubmit();

  if (!listing) {
    return listError ? <FormError error={listError} /> : null;
  }
  const act = (work: () => Promise<unknown>) => {
    setError(null);
    work().catch((reason: unknown) => setError(toCommandError(reason).message));
  };
  const current = listing.organizations.find((o) => o.here);
  const shownName = name ?? current?.name ?? "";

  const saveName = (e: FormEvent) => {
    e.preventDefault();
    void rename.run(async () => {
      try {
        org.apply(await renameOrganization(shownName.trim()));
        setName(null);
        return null;
      } catch (reason) {
        return toCommandError(reason).message;
      }
    });
  };

  return (
    <div className="organizations-setting">
      <form
        className="organizations-setting__name"
        aria-label="This organization's name"
        onSubmit={saveName}
      >
        <Field label="This organization's name">
          <input
            value={shownName}
            maxLength={200}
            required
            onChange={(e) => setName(e.target.value)}
          />
        </Field>
        <Button
          type="submit"
          size="sm"
          disabled={rename.pending || name === null || shownName.trim() === ""}
        >
          Rename
        </Button>
        <FormError error={rename.error} />
      </form>

      <h3>Your organizations</h3>
      <p className="muted">
        Each organization has its own org chart, projects, work, approvals, backups, and secrets.
        Your Workforce, your tile, and the AI tools are shared. Each window shows one organization.
      </p>
      <ul className="organizations-setting__list" aria-label="Your organizations">
        {listing.organizations.map((o) => (
          <li key={o.id} className="organizations-setting__row">
            <div className="organizations-setting__who">
              <strong>{o.name}</strong>
              <StatusPill
                status={o.archived ? "offline" : o.here || o.inWindow ? "ok" : "pending"}
                label={where(o)}
              />
              {o.first && <span className="muted">Your first organization</span>}
            </div>
            <div className="organizations-setting__actions">
              {!o.archived && !o.here && (
                <>
                  {!o.inWindow && (
                    <Button size="sm" onClick={() => act(() => switchOrganization(o.id))}>
                      Switch to
                    </Button>
                  )}
                  <Button
                    size="sm"
                    icon="external"
                    onClick={() => act(() => openOrganizationWindow(o.id))}
                  >
                    {o.inWindow ? "Show its window" : "Open in a new window"}
                  </Button>
                </>
              )}
              {!o.archived && !o.first && !o.here && (
                <Button
                  size="sm"
                  variant="quiet"
                  onClick={() => act(async () => apply(await archiveOrganization(o.id)))}
                >
                  Archive organization
                </Button>
              )}
              {o.archived && (
                <>
                  <Button
                    size="sm"
                    onClick={() => act(async () => apply(await bringBackOrganization(o.id)))}
                  >
                    Bring back
                  </Button>
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() =>
                      act(async () => setDeleting(await previewDeleteOrganization(o.id)))
                    }
                  >
                    Delete for good
                  </Button>
                </>
              )}
            </div>
          </li>
        ))}
      </ul>
      <FormError error={error} />
      {onFree && (
        <p className="muted" role="note">
          More than one organization is part of Plenipo Pro (Settings → License). Free keeps one; an
          archived organization waits, kept, until Pro is back.
        </p>
      )}
      {atPlanLimit && (
        <p className="muted" role="note">
          Your plan covers {license.organizationsCovered} organizations, and all are in use. Plenipo
          Partner plans cover 10, 25, or any number, for companies that run Plenipo for clients
          (Settings → License).
        </p>
      )}
      <div className="settings-section__actions">
        <Button size="sm" icon="plus" onClick={() => setCreating(true)}>
          New organization
        </Button>
      </div>
      {creating && (
        <NewOrganizationDialog
          listing={listing}
          onClose={() => setCreating(false)}
          onCreated={(made) => {
            setCreating(false);
            act(() => openOrganizationWindow(made.id));
          }}
        />
      )}
      {deleting && (
        <DeleteOrganizationDialog
          preview={deleting}
          onClose={() => setDeleting(null)}
          onDeleted={(next) => {
            setDeleting(null);
            apply(next);
          }}
        />
      )}
    </div>
  );
}

/**
 * Delete for good (ADR-094 §18): asks first, and offers to save its experienced workers to your
 * Workforce, as deleting a department or project does (ADR-045 §8).
 */
function DeleteOrganizationDialog({
  preview,
  onClose,
  onDeleted,
}: {
  preview: OrgDeletePreview;
  onClose: () => void;
  onDeleted: (next: Awaited<ReturnType<typeof deleteOrganizationForGood>>) => void;
}) {
  const [save, setSave] = useState<Set<string>>(
    () => new Set(preview.experienced.map((w) => w.positionId)),
  );
  const [sure, setSure] = useState(false);
  const { pending, error, run } = useSubmit();
  const toggle = (id: string) =>
    setSave((s) => {
      const next = new Set(s);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  const submit = (e: FormEvent) => {
    e.preventDefault();
    void run(async () => {
      try {
        onDeleted(await deleteOrganizationForGood(preview.id, [...save]));
        return null;
      } catch (reason) {
        return toCommandError(reason).message;
      }
    });
  };
  return (
    <Modal title={`Delete ${preview.name} for good?`} onClose={onClose}>
      <form className="modal__body" aria-label="Delete organization for good" onSubmit={submit}>
        <p>
          This removes {preview.name}&apos;s Ledger, its backups, its working copies, screenshots,
          and website sign-ins, and forgets its secrets. It cannot be undone. Project folders you
          chose stay where they are.
        </p>
        {preview.experienced.length > 0 && (
          <fieldset className="choices choices--stack">
            <legend>Save its experienced workers to your Workforce</legend>
            {preview.experienced.map((w) => (
              <label key={w.positionId} className="choice">
                <input
                  type="checkbox"
                  checked={save.has(w.positionId)}
                  onChange={() => toggle(w.positionId)}
                />
                <span className="choice__text">
                  <span className="choice__label">{w.title}</span>
                  <span className="muted">
                    {w.roleName} · {w.tasksDone} task{w.tasksDone === 1 ? "" : "s"} done ·{" "}
                    {w.keptLessons} lesson{w.keptLessons === 1 ? "" : "s"} kept
                  </span>
                </span>
              </label>
            ))}
          </fieldset>
        )}
        {preview.notSaved.length > 0 && (
          <p className="muted">
            Can&apos;t be saved, because your first organization has no role of the same name:{" "}
            {preview.notSaved.map((w) => `${w.title} (${w.roleName})`).join(", ")}.
          </p>
        )}
        <label className="choice">
          <input type="checkbox" checked={sure} onChange={(e) => setSure(e.target.checked)} />
          <span className="choice__label">Yes, delete {preview.name} for good</span>
        </label>
        <FormError error={error} />
        <Footer pending={pending} label="Delete for good" disabled={!sure} onCancel={onClose} />
      </form>
    </Modal>
  );
}
