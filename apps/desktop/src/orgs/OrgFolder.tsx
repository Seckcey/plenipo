import { useCallback, useEffect, useState } from "react";
import type { OrgFolderInfo } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { getOrgFolder, openOrgFolder, toCommandError } from "../api/commands";
import { FormError } from "../components/org/OrgDialogs";
import { folderName, needsKeepAlert } from "./orgFolderWords";

/**
 * The alert, in plain words, when a sync service keeps the organization folder online (ADR-205
 * §2.1): files kept only online can't be read by workers until they download, so the owner sets
 * the folder to stay on this computer.
 */
export function KeepOnThisDeviceAlert({
  info,
  onShow,
  onCheck,
}: {
  info: OrgFolderInfo | null;
  /** Show the folder in File Explorer (only once it is there). */
  onShow?: (() => void) | undefined;
  /** Look again whether it is set to stay on this device. */
  onCheck?: (() => void) | undefined;
}) {
  if (!needsKeepAlert(info) || !info?.path) return null;
  const name = folderName(info.path);
  return (
    <div className="notice-box org-folder__alert" role="note" aria-label="Keep it on this computer">
      {info.syncedBy === "oneDrive" ? (
        <p>
          Your organization folder is in OneDrive. OneDrive can keep files only online, and your
          workers can&apos;t read those until they download. In File Explorer, right-click the{" "}
          <strong>{name}</strong> folder and choose <strong>Always keep on this device</strong>.
        </p>
      ) : (
        <p>
          Your organization folder is in a folder that another service syncs online, like Dropbox,
          Google Drive, or iCloud. Set that service to keep the <strong>{name}</strong> folder on
          this computer (on a Mac, <strong>Keep Downloaded</strong>), so your workers can read its
          files.
        </p>
      )}
      {(onShow || onCheck) && (
        <div className="org-folder__alert-actions">
          {onShow && (
            <Button size="sm" variant="secondary" onClick={onShow}>
              Show in folder
            </Button>
          )}
          {onCheck && info.syncedBy === "oneDrive" && (
            <Button size="sm" variant="secondary" onClick={onCheck}>
              Check again
            </Button>
          )}
        </div>
      )}
    </div>
  );
}

/**
 * Settings → Organization: this organization's folder (ADR-205). Where it is, Show in folder,
 * what is backed up, and the alert when a sync service keeps it online.
 */
export function OrgFolderSetting() {
  const [info, setInfo] = useState<OrgFolderInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    getOrgFolder()
      .then((i) => {
        setInfo(i);
        setError(null);
      })
      .catch((reason: unknown) => setError(toCommandError(reason).message));
  }, []);
  useEffect(load, [load]);

  const show = () => {
    setError(null);
    openOrgFolder().catch((reason: unknown) => setError(toCommandError(reason).message));
  };

  if (!info) return error ? <FormError error={error} /> : null;
  return (
    <section className="org-folder" aria-label="Organization folder">
      <h3>Organization folder</h3>
      {info.path ? (
        <>
          <div className="org-folder__where">
            <code className="org-folder__path">{info.path}</code>
            {info.exists && (
              <Button size="sm" icon="external" onClick={show}>
                Show in folder
              </Button>
            )}
          </div>
          {!info.exists && (
            <p className="muted">
              It isn&apos;t there right now. Plenipo makes it again at the same place.
            </p>
          )}
          <p className="muted">
            It holds a folder for each department and project, with their finished files, and a
            scratch pad for each worker. Plenipo backs up its record of this organization every day.
            This folder is ordinary files: your usual backup covers it (OneDrive or File History,
            for example). Plenipo doesn&apos;t copy it.
            {info.syncedBy === "oneDrive" && " OneDrive keeps a copy online."}
          </p>
          <KeepOnThisDeviceAlert
            info={info}
            onShow={info.exists ? show : undefined}
            onCheck={load}
          />
        </>
      ) : (
        <p className="muted">
          This organization has no organization folder yet. Organizations you make from now on get
          one.
        </p>
      )}
      <FormError error={error} />
    </section>
  );
}
