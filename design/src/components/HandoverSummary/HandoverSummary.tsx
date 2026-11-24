import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

/** The handover written at a checkpoint, opened from its divider (X31). */
export function HandoverSummary({ className, children, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div className={cx("flex flex-col gap-4 rounded-md bg-surface px-12 py-12", className)} {...rest}>
      <p className="type-ui-strong text-fg">Handover summary</p>
      <div className="type-ui-para text-soft">{children}</div>
    </div>
  );
}
