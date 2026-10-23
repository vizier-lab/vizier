import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

/** The model's own reasoning, quoted on the trail (D53). Two thoughts in a row merge into one. */
export function Thought({ className, ...rest }: HTMLAttributes<HTMLQuoteElement>) {
  return <blockquote className={cx("border-l-2 border-border-strong py-2 pl-12 type-ui-para text-muted", className)} {...rest} />;
}
