import { useCallback, useId, useRef, useState, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { Badge } from "../Badge/Badge";
import { FieldFrame, fieldBox } from "../TextField/Field";
import { ListboxPopup, useOutsideClose, type ListboxOption } from "./Listbox";

export type SelectProps<V extends string> = {
  label?: ReactNode;
  /** Required when there's no visible label (the role select in a table). */
  "aria-label"?: string;
  options: ListboxOption<V>[];
  value?: V;
  onChange: (value: V) => void;
  placeholder?: string;
  helper?: ReactNode;
  invalid?: boolean;
  disabled?: boolean;
  /** Model ids, protocols: JetBrains Mono. */
  mono?: boolean;
  /** Field (forms) or a compact badge (the role in a Users row). */
  look?: "field" | "badge";
  className?: string;
};

/** A dropdown for short fixed lists (D14). Long or searchable lists use Picker. */
export function Select<V extends string>({
  label,
  "aria-label": ariaLabel,
  options,
  value,
  onChange,
  placeholder = "Choose…",
  helper,
  invalid,
  disabled,
  mono,
  look = "field",
  className,
}: SelectProps<V>) {
  const id = useId();
  const ids = { input: `${id}-button`, helper: `${id}-helper` };
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const close = useCallback(() => setOpen(false), []);
  useOutsideClose(open, root, close);
  const current = options.find((o) => o.value === value);
  const name = ariaLabel ?? (typeof label === "string" ? label : "Choose");

  const trigger =
    look === "badge" ? (
      <button
        ref={button}
        id={ids.input}
        type="button"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? `${id}-list` : undefined}
        aria-label={ariaLabel}
        onClick={() => setOpen(!open)}
        className="inline-flex items-center gap-4 rounded-xs disabled:opacity-40"
      >
        <Badge tone="rank">{current?.label ?? placeholder}</Badge>
        <Icon name="chevron-down" size={12} tone="faint" />
      </button>
    ) : (
      <button
        ref={button}
        id={ids.input}
        type="button"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? `${id}-list` : undefined}
        aria-label={label ? undefined : ariaLabel}
        aria-invalid={invalid || undefined}
        aria-describedby={helper ? ids.helper : undefined}
        onClick={() => setOpen(!open)}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" || e.key === "ArrowUp") {
            e.preventDefault();
            setOpen(true);
          }
        }}
        className={cx(
          fieldBox({ invalid, disabled }),
          "h-40 justify-between gap-8 pr-10 pl-12 text-left focus-visible:border-live focus-visible:outline-none",
          open && "border-border-strong",
          disabled && "cursor-not-allowed",
        )}
      >
        <span className={cx("flex min-w-0 items-center gap-8 truncate", mono ? "type-code" : "type-body", !current && "text-faint")}>
          {current?.leading}
          {current?.label ?? placeholder}
        </span>
        <Icon name="chevron-down" size={14} tone="muted" />
      </button>
    );

  const control = (
    <div ref={root} className="relative">
      {trigger}
      {open && (
        <ListboxPopup
          anchor={root}
          matchWidth
          id={`${id}-list`}
          label={name}
          options={options}
          value={value}
          onPick={(v) => {
            onChange(v);
            setOpen(false);
            button.current?.focus();
          }}
          onClose={() => {
            setOpen(false);
            button.current?.focus();
          }}
        />
      )}
    </div>
  );

  if (look === "badge") return <div className={className}>{control}</div>;
  return (
    <FieldFrame label={label} helper={helper} invalid={invalid} ids={ids} className={className}>
      {control}
    </FieldFrame>
  );
}
