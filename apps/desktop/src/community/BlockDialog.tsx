import { blockInCommunity, toCommandError } from "../api/commands";
import { ConfirmDialog } from "../components/org/Modal";
import { blockTitle, blockWords } from "./blockReportWords";

/**
 * **Block** (ADR-167 §1): the question asked before someone is blocked, on a card and in a
 * conversation. They can't message you, find your card, or link with you, and they are not told.
 * Nothing happens until you say so. `name` is their Community name, sent as it came; `onBlocked`
 * is told once it is done.
 */
export function BlockDialog({
  memberId,
  name,
  onBlocked,
  onCancel,
}: {
  memberId: string;
  name: string;
  onBlocked: () => void;
  onCancel: () => void;
}) {
  return (
    <ConfirmDialog
      title={blockTitle(name)}
      message={<p>{blockWords()}</p>}
      confirmLabel="Block"
      danger
      onCancel={onCancel}
      onConfirm={async () => {
        try {
          await blockInCommunity(memberId, name);
        } catch (reason) {
          return toCommandError(reason).message;
        }
        onBlocked();
        return null;
      }}
    />
  );
}
