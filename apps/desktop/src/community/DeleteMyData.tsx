import { useState } from "react";
import { Button } from "@plenipo/ui";

import { deleteMyCommunityData, toCommandError } from "../api/commands";
import { ConfirmDialog } from "../components/org/Modal";
import { systemWords } from "../system/words";
import { DELETED, deleteDataLabel, deleteDataWords } from "./blockReportWords";

/**
 * **Delete my Community data from this PC** (ADR-167 §15, ADR-168 §3): every Community
 * conversation and message on this computer, after asking. Nothing is sent: 8 West and the other
 * people keep theirs. It is there whether or not you are signed in, because what is on this
 * computer stays until you delete it.
 */
export function DeleteMyData() {
  const [asking, setAsking] = useState(false);
  const [deleted, setDeleted] = useState(false);
  return (
    <section className="community-delete">
      <p className="muted">
        Conversations and messages stay on {systemWords().thisComputer} until you delete them.
      </p>
      <div className="settings-section__actions">
        <Button
          variant="danger"
          onClick={() => {
            setDeleted(false);
            setAsking(true);
          }}
        >
          {deleteDataLabel()}
        </Button>
      </div>
      {deleted && <p role="status">{DELETED}</p>}
      {asking && (
        <ConfirmDialog
          title={`${deleteDataLabel()}?`}
          message={<p>{deleteDataWords()}</p>}
          confirmLabel="Delete my Community data"
          danger
          onCancel={() => setAsking(false)}
          onConfirm={async () => {
            try {
              await deleteMyCommunityData();
            } catch (reason) {
              return toCommandError(reason).message;
            }
            setAsking(false);
            setDeleted(true);
            return null;
          }}
        />
      )}
    </section>
  );
}
