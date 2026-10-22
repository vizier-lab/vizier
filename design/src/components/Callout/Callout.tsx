import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type CalloutProps = {
  /** A medium-weight first line ("That's every addon in this version."). */
  title?: ReactNode;
  /** A right-aligned action. */
  action?: ReactNode;
} & HTMLAttributes<HTMLDivElement>;

/** A quiet dashed hint (X5): what else to try, or why the list ends here. */
export function Callout({ title, action, className, children, ...rest }: CalloutProps) {
  return (
    <div
      className={cx(
        "flex items-center gap-12 border border-dashed border-border-strong",
        title ? "rounded-xl px-20 py-16" : "rounded-lg px-16 py-12",
        className,
      )}
      {...rest}
    >
      <div className="flex min-w-0 flex-1 flex-col gap-6">
        {title && <p className="type-body-medium text-fg">{title}</p>}
        <p className="type-ui text-muted">{children}</p>
      </div>
      {action}
    </div>
  );
}
