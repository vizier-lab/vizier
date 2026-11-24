import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type ActionBarProps = {
  /** Bottom: a wizard's Back / Continue. Top: an editor's breadcrumb … unsaved, Discard, Save. */
  position?: "bottom" | "top";
  start?: ReactNode;
  end?: ReactNode;
} & HTMLAttributes<HTMLDivElement>;

/** A sticky bar with a left and a right group of actions (X4). */
export function ActionBar({ position = "bottom", start, end, className, ...rest }: ActionBarProps) {
  return (
    <div
      className={cx(
        "sticky z-10 flex min-h-68 items-center justify-between gap-16 bg-sidebar px-32",
        position === "bottom" ? "bottom-0 border-t border-border" : "top-0 border-b border-border",
        className,
      )}
      {...rest}
    >
      <div className="flex min-w-0 items-center gap-10">{start}</div>
      <div className="flex shrink-0 items-center gap-12">{end}</div>
    </div>
  );
}
