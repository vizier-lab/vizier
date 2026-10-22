import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

/** Inline code in replies and descriptions (D42). */
export function InlineCode({ className, ...rest }: HTMLAttributes<HTMLElement>) {
  return <code className={cx("rounded-xs bg-raised px-4 py-px type-code text-soft", className)} {...rest} />;
}
