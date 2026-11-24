import { useEffect, useId, useState, type HTMLAttributes } from "react";
import { cx } from "../../lib/cx";
import { IconButton } from "../IconButton/IconButton";

const HEX = /^#[0-9a-fA-F]{6}$/;
const SHORT_HEX = /^#[0-9a-fA-F]{3}$/;

/** What was typed as lowercase #rrggbb, or null if it isn't a colour yet. */
function parse(input: string): string | null {
  const value = input.trim();
  if (SHORT_HEX.test(value)) return `#${[...value.slice(1)].map((c) => c + c).join("")}`.toLowerCase();
  return HEX.test(value) ? value.toLowerCase() : null;
}

export type ColorFieldProps = {
  label: string;
  /** Always a #rrggbb colour. */
  value: string;
  /** Called only with a whole, valid colour, so what was typed halfway never reaches the owner. */
  onChange: (hex: string) => void;
  /** This value was set by hand and holds when the colours it would follow change. */
  overridden?: boolean;
  /** Goes back to the value it would otherwise have. Needs `overridden`. */
  onReset?: () => void;
} & Omit<HTMLAttributes<HTMLDivElement>, "onChange">;

/**
 * One colour: the platform's picker beside a field to type it in, which also carries the day on a
 * webview whose picker is limited. Shown in an error state until what was typed is a colour.
 */
export function ColorField({ label, value, onChange, overridden, onReset, className, ...rest }: ColorFieldProps) {
  const id = useId();
  const [typed, setTyped] = useState(value);
  const [focused, setFocused] = useState(false);
  // A change from elsewhere (a key colour moved this role) shows at once, unless it is being typed in.
  useEffect(() => {
    if (!focused) setTyped(value);
  }, [value, focused]);
  const invalid = parse(typed) === null;
  return (
    <div className={cx("flex min-w-0 flex-col gap-4", className)} {...rest}>
      <div className="flex min-h-16 items-center gap-4">
        <label htmlFor={id} className="truncate type-small text-soft">
          {label}
        </label>
        {overridden && <span className="type-mono-xs text-faint">(edited)</span>}
      </div>
      <div className="flex items-center gap-6">
        <input
          type="color"
          aria-label={`${label} picker`}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="size-28 shrink-0 cursor-pointer rounded-sm border border-border-strong bg-transparent p-0"
        />
        <input
          id={id}
          type="text"
          spellCheck={false}
          autoComplete="off"
          aria-invalid={invalid || undefined}
          value={typed}
          onFocus={() => setFocused(true)}
          onBlur={() => setFocused(false)}
          onChange={(e) => {
            setTyped(e.target.value);
            const hex = parse(e.target.value);
            if (hex) onChange(hex);
          }}
          className={cx(
            "h-28 w-full min-w-0 rounded-sm border bg-raised px-6 type-mono-xs text-fg outline-none",
            invalid ? "border-failed" : "border-border-strong focus:border-live",
          )}
        />
        {overridden && onReset && <IconButton icon="restore" aria-label={`Reset ${label}`} size="sm" onClick={onReset} />}
      </div>
    </div>
  );
}
