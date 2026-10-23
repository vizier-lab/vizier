import type { ButtonHTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type SegmentProps = {
  selected?: boolean;
  /** Mono count after the label. */
  count?: ReactNode;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** One option of FilterTabs (D16). */
export function Segment({ selected, count, type = "button", className, children, ...rest }: SegmentProps) {
  return (
    <button
      type={type}
      aria-pressed={selected ?? false}
      className={cx(
        "inline-flex h-25 shrink-0 items-center gap-4 rounded-sm px-8 type-small-medium whitespace-nowrap disabled:opacity-40",
        selected ? "bg-raised text-fg" : "text-muted enabled:hover:text-fg enabled:active:bg-pressed",
        className,
      )}
      {...rest}
    >
      {children}
      {count != null && <span className={cx("type-mono-xs", selected ? "text-muted" : "text-faint")}>{count}</span>}
    </button>
  );
}
