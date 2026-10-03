import type { CommunityView } from "@plenipo/types";

import { setCommunitySwitch, toCommandError } from "../api/commands";
import { ConfirmDialog } from "../components/org/Modal";
import { systemWords } from "../system/words";

/**
 * Leave Community (ADR-167 §15): the question asked before the switch goes off, or before the
 * button is pressed. Leaving cannot be undone from here, so nothing happens until you say so.
 * `onLeft` gets Community as it is after leaving.
 */
export function LeaveCommunityDialog({
  onLeft,
  onCancel,
}: {
  onLeft: (view: CommunityView) => void;
  onCancel: () => void;
}) {
  return (
    <ConfirmDialog
      title="Leave Community?"
      message={
        <p>
          Leaving deletes your profile, your listing, and anything still waiting for you at 8 West,
          and signs out every computer of yours. What is on {systemWords().thisComputer} stays until
          you delete it.
        </p>
      }
      confirmLabel="Leave Community"
      danger
      onCancel={onCancel}
      onConfirm={async () => {
        try {
          onLeft(await setCommunitySwitch(false));
          return null;
        } catch (reason) {
          return toCommandError(reason).message;
        }
      }}
    />
  );
}
