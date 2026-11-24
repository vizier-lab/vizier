import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

/** Where a turn was stopped (X32): "Stopped by you at 14:03 · 1 queued message dropped". */
export function StoppedNote({ className, children, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div className={cx("flex items-center gap-8 type-ui text-muted", className)} {...rest}>
      <span aria-hidden className="size-8 rounded-xs bg-unlit" />
      {children}
    </div>
  );
}
