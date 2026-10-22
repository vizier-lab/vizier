import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type InlineSelectProps = {
  /** "Vision model". */
  label: ReactNode;
  /** The current choice: "Anthropic · claude-haiku-5-5". */
  value: ReactNode;
  /** Whether its menu is open; the caller renders the menu. */
  open?: boolean;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** A choice shown as text with a ▾ (X13); opens the caller's menu or picker. */
export function InlineSelect({ label, value, open, type = "button", className, ...rest }: InlineSelectProps) {
  return (
    <button
      type={type}
      aria-haspopup="listbox"
      aria-expanded={open ?? false}
      className={cx("inline-flex items-center gap-4 rounded-xs type-ui-medium text-fg hover:underline underline-offset-2 disabled:opacity-40", className)}
      {...rest}
    >
      <span>
        {label}: {value}
      </span>
      <Icon name="chevron-down" size={13} className="text-muted" />
    </button>
  );
}
