import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type UnsavedIndicatorProps = {
  /** "Unsaved changes", or "Unsaved · draft kept in this browser". */
  children?: ReactNode;
} & HTMLAttributes<HTMLSpanElement>;

/** An amber dot and a note that there are unsaved edits (X8). */
export function UnsavedIndicator({ children = "Unsaved changes", className, ...rest }: UnsavedIndicatorProps) {
  return (
    <span role="status" className={cx("inline-flex items-center gap-6 type-ui text-muted", className)} {...rest}>
      <span aria-hidden className="size-6 rounded-pill bg-retrying" />
      {children}
    </span>
  );
}
