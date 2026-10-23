import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

/** The model's own reasoning, quoted on the trail (D53). A list item, so it sits on the spine between calls. */
export function Thought({ className, children, ...rest }: HTMLAttributes<HTMLQuoteElement>) {
  return (
    <li className="py-4 pl-24">
      <blockquote className={cx("border-l-2 border-border-strong py-2 pl-12 type-ui-para text-muted", className)} {...rest}>
        {children}
      </blockquote>
    </li>
  );
}
