import { useState } from "react";
import { Button } from "@plenipo/ui";

import { deletePlenipoData, toCommandError } from "../api/commands";
import { ConfirmDialog } from "../components/org/Modal";
import { systemWords } from "../system/words";

/**
 * **Delete my Plenipo data** (Phase 23), on a Mac and Linux, where removing Plenipo never
 * touches your home folder: every organization's Ledger, settings, and backups, and the keys
 * Plenipo saved, after asking. Plenipo quits when it is done. Windows' uninstaller has its own
 * tick box for this.
 */
export function DeletePlenipoData() {
  const words = systemWords();
  const [asking, setAsking] = useState<"first" | "stopWork" | null>(null);
  if (words.system === "windows") return null;
  const removeIt =
    words.system === "mac" ? "drag Plenipo to the Trash" : "remove it with your software manager";
  return (
    <section aria-labelledby="delete-plenipo-data" className="settings-delete">
      <h3 id="delete-plenipo-data">Delete my Plenipo data</h3>
      <p className="muted">
        This deletes everything Plenipo keeps on {words.thisComputer}: every organization&apos;s
        Ledger, settings, and backups, and the keys it saved in {words.keyStore}. It can&apos;t be
        undone, and Plenipo quits when it&apos;s done. To remove Plenipo itself afterwards,{" "}
        {removeIt}.
      </p>
      <div className="settings-section__actions">
        <Button variant="danger" onClick={() => setAsking("first")}>
          Delete my Plenipo data
        </Button>
      </div>
      {asking && (
        <ConfirmDialog
          title={asking === "stopWork" ? "Work is running" : "Delete everything and quit?"}
          message={
            <p>
              {asking === "stopWork"
                ? "Deleting your data stops the work that is running, the same way Quit does."
                : "Everything Plenipo keeps on this computer is deleted, and Plenipo quits. This can't be undone."}
            </p>
          }
          confirmLabel={
            asking === "stopWork" ? "Stop the work, delete, and quit" : "Delete everything and quit"
          }
          danger
          onCancel={() => setAsking(null)}
          onConfirm={async () => {
            try {
              await deletePlenipoData(asking === "stopWork");
            } catch (reason) {
              const message = toCommandError(reason).message;
              if (asking === "first" && message.startsWith("Work is running")) {
                setAsking("stopWork");
                return null;
              }
              return message;
            }
            return null;
          }}
        />
      )}
    </section>
  );
}
