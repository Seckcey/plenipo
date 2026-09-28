/**
 * Building blocks of the properties panel (Phase 17): every option carries a one-line "what this
 * does" under it, joined to the control with `aria-describedby`, so it is read out with it.
 */
import { useId, type ReactNode } from "react";
import type { TaskBrief } from "@plenipo/types";
import { Button, StatusPill, type ButtonVariant, type IconName } from "@plenipo/ui";

import { TASK_STATE_LABEL } from "../../../ledger/format";
import { ago } from "../../../org/format";
import { TASK_TONE } from "../../tones";

export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="inspector__section">
      <h3>{title}</h3>
      {children}
    </section>
  );
}

export function Refusal({ error }: { error: string | null }) {
  return error ? (
    <p className="form-error" role="alert">
      {error}
    </p>
  ) : null;
}

export function Fact({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

/** A button with the line that says what it does under it. */
export function Option({
  label,
  hint,
  onClick,
  variant = "quiet",
  disabled = false,
  icon,
  ariaLabel,
}: {
  label: ReactNode;
  hint: string;
  onClick: () => void;
  variant?: ButtonVariant;
  disabled?: boolean;
  icon?: IconName;
  /** A longer spoken name, when the label alone is not clear out of context. */
  ariaLabel?: string;
}) {
  const id = useId();
  return (
    <div className="option">
      <Button
        variant={variant}
        size="sm"
        disabled={disabled}
        aria-describedby={id}
        aria-label={ariaLabel}
        onClick={onClick}
        {...(icon ? { icon } : {})}
      >
        {label}
      </Button>
      <span id={id} className="option__hint">
        {hint}
      </span>
    </div>
  );
}

/** A group of options, one under the other. */
export function Options({ children }: { children: ReactNode }) {
  return <div className="options">{children}</div>;
}

/**
 * A labelled control with its line under it. `children` gets the control's ID and the hint's ID
 * (for `aria-describedby`).
 */
export function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint: ReactNode;
  children: (ids: { id: string; hintId: string }) => ReactNode;
}) {
  const id = useId();
  const hintId = `${id}-hint`;
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>
        {label}
      </label>
      {children({ id, hintId })}
      <small id={hintId} className="field__hint">
        {hint}
      </small>
    </div>
  );
}

/** A checkbox with its line under it. */
export function Check({
  label,
  hint,
  checked,
  disabled = false,
  onChange,
}: {
  label: string;
  hint: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (on: boolean) => void;
}) {
  const id = useId();
  return (
    <div className="check-option">
      <label className="check">
        <input
          type="checkbox"
          checked={checked}
          disabled={disabled}
          aria-describedby={id}
          onChange={(e) => onChange(e.target.checked)}
        />
        <span>{label}</span>
      </label>
      <span id={id} className="check__hint">
        {hint}
      </span>
    </div>
  );
}

/** A task in a list: its objective (opens it), state, and when. */
export function TaskRow({
  task,
  onOpen,
  showOwner = false,
}: {
  task: TaskBrief;
  onOpen: (taskId: string) => void;
  showOwner?: boolean;
}) {
  return (
    <div className="inspector__task">
      <button type="button" className="link" data-nav onClick={() => onOpen(task.id)}>
        {task.objective || "(no objective)"}
      </button>
      <StatusPill status={TASK_TONE[task.state]} label={TASK_STATE_LABEL[task.state]} />
      <span className="muted inspector__task-meta">
        {showOwner && task.positionTitle ? `${task.positionTitle} · ` : ""}
        {ago(task.completedAt ?? task.startedAt ?? task.createdAt)}
      </span>
    </div>
  );
}

/** A link to another item on the chart (it selects it); not an option. */
export function ItemLink({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button type="button" className="link" data-nav onClick={onClick}>
      {children}
    </button>
  );
}
