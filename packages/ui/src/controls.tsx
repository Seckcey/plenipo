/** Buttons, switches, checkboxes, fields, and tabs. */

import {
  useEffect,
  useId,
  useRef,
  type ButtonHTMLAttributes,
  type KeyboardEvent,
  type ReactNode,
} from "react";

import { Icon, type IconName } from "./icons";
import { cx } from "./util";

export type ButtonVariant = "primary" | "secondary" | "quiet" | "danger";

export function Button({
  variant = "secondary",
  size = "md",
  icon,
  className,
  children,
  type = "button",
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: "sm" | "md";
  icon?: IconName;
}) {
  return (
    <button
      type={type}
      className={cx(
        "ui-button",
        `ui-button--${variant}`,
        size === "sm" && "ui-button--sm",
        className,
      )}
      {...rest}
    >
      {icon && <Icon name={icon} size={size === "sm" ? 14 : 16} />}
      {children}
    </button>
  );
}

/** A button that shows only an icon; `label` is its spoken name and tooltip. */
export function IconButton({
  icon,
  label,
  className,
  pressed,
  type = "button",
  ...rest
}: Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children" | "aria-label"> & {
  icon: IconName;
  label: string;
  pressed?: boolean;
}) {
  return (
    <button
      type={type}
      className={cx("ui-icon-button", className)}
      aria-label={label}
      aria-pressed={pressed}
      title={label}
      {...rest}
    >
      <Icon name={icon} size={16} />
    </button>
  );
}

/** An on/off switch. The words "On" and "Off" are shown next to it (not color alone). */
export function Switch({
  checked,
  onChange,
  label,
  disabled,
  showState = true,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  disabled?: boolean;
  showState?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      className={cx("ui-switch", checked && "ui-switch--on")}
      onClick={() => onChange(!checked)}
    >
      <span className="ui-switch__track" aria-hidden="true">
        <span className="ui-switch__thumb" />
      </span>
      {showState && <span className="ui-switch__state">{checked ? "On" : "Off"}</span>}
    </button>
  );
}

export function Checkbox({
  checked,
  indeterminate = false,
  onChange,
  label,
  hideLabel = false,
  count,
  disabled,
}: {
  checked: boolean;
  indeterminate?: boolean;
  onChange: (next: boolean) => void;
  label: string;
  /** Keep the label for screen readers only (row checkboxes). */
  hideLabel?: boolean;
  /** Shown after the label, e.g. "Online (7)". */
  count?: number;
  disabled?: boolean;
}) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = indeterminate;
  }, [indeterminate]);
  return (
    <label className={cx("ui-check", disabled && "ui-check--disabled")}>
      <input
        ref={ref}
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className={hideLabel ? "ui-visually-hidden" : "ui-check__label"}>
        {label}
        {count !== undefined && (
          <>
            {" "}
            <span className="ui-check__count ui-num">({count})</span>
          </>
        )}
      </span>
    </label>
  );
}

export function SearchField({
  value,
  onChange,
  label = "Search",
  placeholder = "Search",
}: {
  value: string;
  onChange: (next: string) => void;
  label?: string;
  placeholder?: string;
}) {
  return (
    <label className="ui-search">
      <Icon name="search" size={14} />
      <span className="ui-visually-hidden">{label}</span>
      <input
        type="search"
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
      />
    </label>
  );
}

export function TextField({
  label,
  value,
  onChange,
  hint,
  type = "text",
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (next: string) => void;
  hint?: ReactNode;
  type?: "text" | "number" | "url";
  placeholder?: string;
}) {
  const id = useId();
  return (
    <div className="ui-field">
      <label htmlFor={id}>{label}</label>
      <input
        id={id}
        type={type}
        value={value}
        placeholder={placeholder}
        aria-describedby={hint ? `${id}-hint` : undefined}
        onChange={(e) => onChange(e.target.value)}
      />
      {hint && (
        <div id={`${id}-hint`} className="ui-field__hint">
          {hint}
        </div>
      )}
    </div>
  );
}

export function Select<T extends string>({
  label,
  value,
  options,
  onChange,
  hideLabel = false,
}: {
  label: string;
  value: T;
  options: readonly { value: T; label: string; group?: string | undefined }[];
  onChange: (next: T) => void;
  hideLabel?: boolean;
}) {
  const id = useId();
  const groups = [...new Set(options.map((o) => o.group ?? ""))];
  return (
    <div className={cx("ui-field", "ui-field--select", hideLabel && "ui-field--bare")}>
      <label htmlFor={id} className={hideLabel ? "ui-visually-hidden" : undefined}>
        {label}
      </label>
      <select id={id} value={value} onChange={(e) => onChange(e.target.value as T)}>
        {groups.map((g) =>
          g ? (
            <optgroup key={g} label={g}>
              {options
                .filter((o) => o.group === g)
                .map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
            </optgroup>
          ) : (
            options
              .filter((o) => !o.group)
              .map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))
          ),
        )}
      </select>
    </div>
  );
}

/** Two to four choices side by side (e.g. Cards / List). */
export function Segmented<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: readonly { value: T; label: string; icon?: IconName; iconOnly?: boolean }[];
  onChange: (next: T) => void;
}) {
  return (
    <div className="ui-segmented" role="group" aria-label={label}>
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          aria-pressed={value === o.value}
          aria-label={o.iconOnly ? o.label : undefined}
          title={o.iconOnly ? o.label : undefined}
          onClick={() => onChange(o.value)}
        >
          {o.icon && <Icon name={o.icon} size={14} />}
          {!o.iconOnly && o.label}
        </button>
      ))}
    </div>
  );
}

/** Tabs with arrow-key movement between them (WAI-ARIA tabs pattern). */
export function Tabs<T extends string>({
  label,
  value,
  tabs,
  onChange,
  idPrefix,
}: {
  label: string;
  value: T;
  tabs: readonly { value: T; label: ReactNode; badge?: ReactNode }[];
  onChange: (next: T) => void;
  /** Tab `i` gets id `${idPrefix}-tab-${value}` and controls `${idPrefix}-panel-${value}`. */
  idPrefix?: string;
}) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const move = (e: KeyboardEvent, index: number) => {
    const last = tabs.length - 1;
    const next =
      e.key === "ArrowRight"
        ? index === last
          ? 0
          : index + 1
        : e.key === "ArrowLeft"
          ? index === 0
            ? last
            : index - 1
          : e.key === "Home"
            ? 0
            : e.key === "End"
              ? last
              : null;
    if (next === null) return;
    e.preventDefault();
    const tab = tabs[next];
    if (tab) onChange(tab.value);
    refs.current[next]?.focus();
  };
  return (
    <div className="ui-tabs" role="tablist" aria-label={label}>
      {tabs.map((t, i) => (
        <button
          key={t.value}
          ref={(el) => {
            refs.current[i] = el;
          }}
          type="button"
          role="tab"
          id={idPrefix ? `${idPrefix}-tab-${t.value}` : undefined}
          aria-controls={idPrefix ? `${idPrefix}-panel-${t.value}` : undefined}
          aria-selected={value === t.value}
          tabIndex={value === t.value ? 0 : -1}
          className="ui-tabs__tab"
          onClick={() => onChange(t.value)}
          onKeyDown={(e) => move(e, i)}
        >
          {t.label}
          {t.badge}
        </button>
      ))}
    </div>
  );
}
