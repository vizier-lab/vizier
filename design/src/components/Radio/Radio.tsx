import type { InputHTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type RadioProps = {
  label: ReactNode;
} & Omit<InputHTMLAttributes<HTMLInputElement>, "type">;

/** One of a few options shown together (D20). Group radios with the same `name`. */
export function Radio({ label, className, disabled, ...rest }: RadioProps) {
  return (
    <label className={cx("inline-flex items-center gap-8 type-ui text-fg", disabled && "opacity-40", className)}>
      <span className="relative inline-flex size-16 shrink-0">
        <input
          type="radio"
          disabled={disabled}
          className={cx(
            "peer size-16 appearance-none rounded-pill border border-border-strong",
            "enabled:hover:border-border-hover checked:border-ink enabled:checked:hover:border-ink-hover",
          )}
          {...rest}
        />
        <span aria-hidden className="pointer-events-none absolute top-4 left-4 hidden size-8 rounded-pill bg-ink peer-checked:block" />
      </span>
      {label}
    </label>
  );
}
