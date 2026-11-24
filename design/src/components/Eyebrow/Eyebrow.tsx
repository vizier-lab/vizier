import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type EyebrowProps = {
  /** Shown after a middle dot: "CONNECTED · 5", "PERMISSIONS · 4 OF 12". */
  count?: ReactNode;
  /** Right-aligned, e.g. a "Show granted only" toggle. */
  action?: ReactNode;
  /** Heading level when the label heads a section; plain text otherwise. */
  as?: "h2" | "h3" | "h4" | "div";
} & HTMLAttributes<HTMLElement>;

/** An uppercase mono section label (X1). */
export function Eyebrow({ count, action, as: As = "div", className, children, ...rest }: EyebrowProps) {
  const text = (
    <>
      {children}
      {count != null && <> · {count}</>}
    </>
  );
  if (!action) {
    return (
      <As className={cx("type-label uppercase text-faint", className)} {...rest}>
        {text}
      </As>
    );
  }
  return (
    <div className={cx("flex items-center justify-between gap-12", className)} {...rest}>
      <As className="type-label uppercase text-faint">{text}</As>
      {action}
    </div>
  );
}
