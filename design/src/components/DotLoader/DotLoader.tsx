import type { HTMLAttributes } from "react";
import type { ToneMode } from "../../tokens/tokens";
import { cx } from "../../lib/cx";

export type DotLoaderProps = {
  /** Colour, like an icon's; inherits the text colour when unset. */
  tone?: ToneMode;
  /** Announced to assistive tech; omit when a visible label says the same. */
  label?: string;
} & HTMLAttributes<HTMLSpanElement>;

/** Three equal dots fading in turn: inline progress (D32). */
export function DotLoader({ tone, label, className, ...rest }: DotLoaderProps) {
  return (
    <span
      role={label ? "status" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      className={cx("inline-flex h-15 shrink-0 items-center gap-2 px-2", tone && `tone-${tone}`, className)}
      {...rest}
    >
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          className="size-4 rounded-pill bg-current animate-dot-wave motion-reduce:animate-none"
          style={{ animationDelay: `${i * 0.2}s`, opacity: [0.35, 0.65, 1][i] }}
        />
      ))}
    </span>
  );
}
