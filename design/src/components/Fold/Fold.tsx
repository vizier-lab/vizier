import type { ButtonHTMLAttributes } from "react";
import { cx } from "../../lib/cx";

export type FoldProps = {
  /** "9 more · 1 thought, 8 python runs". */
  label: string;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** The folded middle of a long trail (D54); it opens in place. */
export function Fold({ label, type = "button", className, ...rest }: FoldProps) {
  return (
    <li className="relative flex min-h-28 items-center gap-10 not-last:after:absolute not-last:after:top-20 not-last:after:-bottom-8 not-last:after:left-6 not-last:after:w-px not-last:after:bg-border-strong not-last:after:content-['']">
      <span aria-hidden className="relative z-10 flex size-12 shrink-0 flex-col items-center justify-center gap-2 bg-surface">
        <span className="size-2 rounded-pill bg-faint" />
        <span className="size-2 rounded-pill bg-faint" />
        <span className="size-2 rounded-pill bg-faint" />
      </span>
      <button
        type={type}
        aria-expanded={false}
        className={cx("h-22 rounded-pill border border-border-strong px-8 type-mono-xs text-muted hover:bg-raised hover:text-fg", className)}
        {...rest}
      >
        {label}
      </button>
    </li>
  );
}
