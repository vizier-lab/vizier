import type { ButtonHTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type JumpToLatestProps = {
  /** New messages below the fold; 0 shows a plain "Jump to latest". */
  count?: number;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** Floats over the conversation when new messages arrive below (X43). */
export function JumpToLatest({ count = 0, type = "button", className, ...rest }: JumpToLatestProps) {
  return (
    <button
      type={type}
      className={cx(
        "inline-flex h-27 items-center gap-6 rounded-pill bg-ink px-12 type-small-strong text-ground shadow-popover hover:bg-ink-hover active:bg-ink-pressed",
        className,
      )}
      {...rest}
    >
      {count > 0 ? `${count} new message${count === 1 ? "" : "s"}` : "Jump to latest"}
      <Icon name="arrow-down" size={13} />
    </button>
  );
}
