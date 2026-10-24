import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

/**
 * The model's own reasoning, quoted on the trail (D53). A list item, so it sits on the spine between
 * calls: the spine runs on through it to the next call, and the text lines up with the call names.
 */
export function Thought({ className, children, ...rest }: HTMLAttributes<HTMLQuoteElement>) {
  return (
    <li className="relative py-4 pl-24 not-last:after:absolute not-last:after:top-0 not-last:after:-bottom-8 not-last:after:left-6 not-last:after:w-px not-last:after:bg-border-strong not-last:after:content-['']">
      <blockquote className={cx("type-ui-para text-muted", className)} {...rest}>
        {children}
      </blockquote>
    </li>
  );
}
