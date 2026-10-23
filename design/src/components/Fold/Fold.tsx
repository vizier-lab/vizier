import type { ButtonHTMLAttributes } from "react";
import { cx } from "../../lib/cx";

export type FoldProps = {
  /** "9 more · 1 thought, 8 python runs". */
  label: string;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** The folded middle of a long trail (D54); it opens in place. */
export function Fold({ label, type = "button", className, ...rest }: FoldProps) {
  return (
    <li className="flex min-h-28 items-center gap-10">
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
