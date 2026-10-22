import type { InputHTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type CheckboxProps = {
  /** Visible label; without one, pass aria-label. */
  label?: ReactNode;
} & Omit<InputHTMLAttributes<HTMLInputElement>, "type">;

/** A checkbox, for choices saved with a button (D19). */
export function Checkbox({ label, className, disabled, ...rest }: CheckboxProps) {
  const box = (
    <span className="relative inline-flex size-16 shrink-0">
      <input
        type="checkbox"
        disabled={disabled}
        className={cx(
          "peer size-16 appearance-none rounded-xs border border-border-strong bg-transparent",
          "enabled:hover:border-border-hover checked:border-ink checked:bg-ink enabled:checked:hover:bg-ink-hover",
          "disabled:opacity-40",
          !label && className,
        )}
        {...rest}
      />
      <Icon
        name="check"
        size={12}
        tone="inverse"
        className="pointer-events-none absolute top-2 left-2 hidden peer-checked:block peer-disabled:opacity-40"
      />
    </span>
  );
  if (!label) return box;
  return (
    <label className={cx("inline-flex items-center gap-8 type-ui text-fg", disabled && "opacity-40", className)}>
      {box}
      {label}
    </label>
  );
}
