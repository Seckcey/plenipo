import { useState } from "react";
import type { RoutingSnapshot } from "@plenipo/types";

import { modelGroups } from "../../routing/format";

/** The menu value that switches to typing a model name (no model name starts with "_"). */
const TYPE_A_NAME = "_type";

export interface ModelPickerProps {
  /** The model settings (short names, your models, models seen in use); `null` while loading. */
  routing: RoutingSnapshot | null;
  runtimeId: string;
  /** The model name the AI tool is given; "" for the AI tool's default. */
  value: string;
  onChange: (name: string) => void;
  label?: string;
  /** Include the owner's own models (not when adding one to that list). */
  yours?: boolean;
  /** Names shown but not offered, with why (for example already in your list). */
  unavailable?: (name: string) => string | null;
  disabled?: boolean;
}

/**
 * Choose the model an AI tool runs: its default, its own short names, your models, the models
 * it reported running, or — as a last resort — a name typed by hand. Starts over when the AI tool
 * changes.
 */
export function ModelPicker(props: ModelPickerProps) {
  return <Picker key={props.runtimeId} {...props} />;
}

function Picker({
  routing,
  runtimeId,
  value,
  onChange,
  label = "Model",
  yours = true,
  unavailable = () => null,
  disabled = false,
}: ModelPickerProps) {
  const groups = modelGroups(routing, runtimeId, { yours });
  const listed = groups.some((g) => g.options.some((o) => o.name === value));
  const [typing, setTyping] = useState(false);
  const custom = typing || (value !== "" && !listed);
  const defaultTaken = unavailable("");

  return (
    <>
      <label className="field">
        <span>{label}</span>
        <select
          value={custom ? TYPE_A_NAME : value}
          disabled={disabled}
          onChange={(e) => {
            const next = e.target.value;
            setTyping(next === TYPE_A_NAME);
            onChange(next === TYPE_A_NAME ? "" : next);
          }}
        >
          <option value="" disabled={defaultTaken !== null && value !== ""}>
            The AI tool&apos;s default{defaultTaken ? ` (${defaultTaken})` : ""}
          </option>
          {groups.map((g) => (
            <optgroup key={g.label} label={g.label}>
              {g.options.map((o) => {
                const why = o.name === value ? null : unavailable(o.name);
                return (
                  <option key={o.name} value={o.name} disabled={why !== null}>
                    {o.label}
                    {why ? ` (${why})` : ""}
                  </option>
                );
              })}
            </optgroup>
          ))}
          <option value={TYPE_A_NAME}>Type another name…</option>
        </select>
      </label>
      {custom && (
        <label className="field">
          <span>Model name the AI tool accepts</span>
          <input
            value={value}
            maxLength={64}
            disabled={disabled}
            placeholder="Blank: the AI tool's default"
            onChange={(e) => onChange(e.target.value)}
          />
          <small className="field__hint">
            Exactly as the AI tool&apos;s own model option takes it.
          </small>
        </label>
      )}
    </>
  );
}
