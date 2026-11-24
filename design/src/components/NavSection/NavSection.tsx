import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type NavSectionProps = {
  /** Uppercase mono label, e.g. "Integrations". */
  label: string;
  /** The 64px rail: a hairline instead of the label. */
  collapsed?: boolean;
  children?: ReactNode;
} & HTMLAttributes<HTMLDivElement>;

/** A sidebar group heading (D3), with the items it heads. */
export function NavSection({ label, collapsed, children, className, ...rest }: NavSectionProps) {
  return (
    <div role="group" aria-label={label} className={cx("flex flex-col", className)} {...rest}>
      {collapsed ? (
        <div aria-hidden className="flex w-44 items-center px-10 py-8">
          <span className="h-px flex-1 bg-border" />
        </div>
      ) : (
        <div aria-hidden className="px-10 pt-12 pb-4 type-label uppercase text-faint">
          {label}
        </div>
      )}
      {children}
    </div>
  );
}
