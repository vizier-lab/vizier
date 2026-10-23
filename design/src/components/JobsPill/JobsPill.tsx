import type { ButtonHTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type JobsPillProps = {
  /** Running jobs; the pill hides itself at zero. */
  count: number;
  /** Mobile: the layers icon and the count only. */
  compact?: boolean;
  /** Whether the tray it opens is open. */
  open?: boolean;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** The count of running background jobs; opens the tray (D60). */
export function JobsPill({ count, compact, open, type = "button", className, ...rest }: JobsPillProps) {
  if (count <= 0) return null;
  const label = `${count} background job${count === 1 ? "" : "s"}`;
  return (
    <button
      type={type}
      aria-expanded={open ?? false}
      aria-label={compact ? label : undefined}
      className={cx(
        "inline-flex h-29 items-center gap-6 rounded-pill border border-border-strong bg-surface hover:bg-raised",
        compact ? "px-10" : "pr-12 pl-10",
        className,
      )}
      {...rest}
    >
      {compact ? <Icon name="layers" size={15} /> : <span aria-hidden className="size-6 rounded-pill bg-live" />}
      <span className={compact ? "type-mono-xs-medium text-fg" : "type-small-medium text-fg"}>{compact ? count : label}</span>
    </button>
  );
}
