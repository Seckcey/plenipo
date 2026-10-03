/**
 * Side chats (Phase 25, item 3.5; ADR-251): **Ask a question** asks a full-time agent something
 * while it works or waits for its team. The answer comes in a new conversation (in Workers, "Side
 * chat with Alex") that knows what the agent knows. It answers only: no tools, no hand-offs, and
 * its work is never touched.
 */
import { useId, useState, type FormEvent } from "react";
import type { PositionInfo } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { askSideQuestion, toCommandError } from "../../api/commands";
import { Modal } from "../org/Modal";
import { Field, Footer, FormError } from "../org/OrgDialogs";
import { canAsk } from "./canAsk";

export function AskQuestionButton({
  p,
  onAsked,
  size = "sm",
  describedBy,
}: {
  p: PositionInfo;
  /** Opens the side chat (in Workers). */
  onAsked: (sessionId: string) => void;
  size?: "sm" | "md";
  describedBy?: string;
}) {
  const [asking, setAsking] = useState(false);
  if (!canAsk(p)) return null;
  return (
    <>
      <Button size={size} aria-describedby={describedBy} onClick={() => setAsking(true)}>
        Ask a question
      </Button>
      {asking && <AskQuestionDialog p={p} onClose={() => setAsking(false)} onAsked={onAsked} />}
    </>
  );
}

function AskQuestionDialog({
  p,
  onClose,
  onAsked,
}: {
  p: PositionInfo;
  onClose: () => void;
  onAsked: (sessionId: string) => void;
}) {
  const [question, setQuestion] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const id = useId();
  const ask = async (e: FormEvent) => {
    e.preventDefault();
    setPending(true);
    setError(null);
    try {
      const detail = await askSideQuestion(p.id, question.trim());
      onClose();
      onAsked(detail.session.id);
    } catch (reason) {
      setError(toCommandError(reason).message);
      setPending(false);
    }
  };
  return (
    <Modal title={`Ask ${p.title} a question`} onClose={onClose} tour="side-chat">
      <form aria-label="Ask a question" onSubmit={(e) => void ask(e)}>
        <div className="modal__body">
          <Field
            label="Your question"
            hint={`${p.title} answers from what it knows, while its work goes on. In a side chat it can't use tools or hand work to its team. The answer opens in Workers.`}
          >
            <textarea
              id={id}
              rows={4}
              maxLength={40_000}
              required
              value={question}
              onChange={(e) => setQuestion(e.target.value)}
            />
          </Field>
          <FormError error={error} />
        </div>
        <Footer
          pending={pending}
          label="Ask"
          disabled={question.trim() === ""}
          onCancel={onClose}
        />
      </form>
    </Modal>
  );
}
