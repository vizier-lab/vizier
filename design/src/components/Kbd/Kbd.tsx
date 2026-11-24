import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

export type KbdProps = {
  /** Inline sits in running text; inside sits in a button or tooltip and takes its colour. */
  look?: "inline" | "inside";
} & HTMLAttributes<HTMLElement>;

/** A key hint such as "Ctrl S" or "Enter" (X6). */
export function Kbd({ look = "inline", className, ...rest }: KbdProps) {
  return (
    <kbd
      className={cx(
        "type-mono-xs whitespace-nowrap",
        look === "inline"
          ? "inline-flex h-18 items-center rounded-xs border border-border-strong bg-surface px-4 text-muted"
          : "opacity-60",
        className,
      )}
      {...rest}
    />
  );
}
