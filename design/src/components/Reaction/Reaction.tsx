import type { ButtonHTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type ReactionProps = {
  emoji: string;
  count: number;
  /** You reacted with this one; clicking removes yours. */
  mine?: boolean;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** One reaction on an agent reply (D55). */
export function Reaction({ emoji, count, mine, type = "button", className, ...rest }: ReactionProps) {
  return (
    <button
      type={type}
      aria-pressed={mine ?? false}
      aria-label={`${emoji} ${count}${mine ? ", including you" : ""}`}
      className={cx(
        "inline-flex h-23 items-center gap-4 rounded-pill border pr-8 pl-8",
        mine ? "border-border-hover bg-pressed" : "border-border-strong bg-surface hover:bg-raised",
        className,
      )}
      {...rest}
    >
      <span aria-hidden className="type-small">{emoji}</span>
      <span aria-hidden className={cx("text-fg", mine ? "type-mono-xs-medium" : "type-mono-xs")}>{count}</span>
    </button>
  );
}

/** The add-reaction button that sits beside the reactions. */
export function AddReaction({ className, type = "button", ...rest }: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type={type}
      aria-label="Add reaction"
      className={cx("inline-flex size-23 items-center justify-center rounded-pill text-faint hover:bg-raised hover:text-fg", className)}
      {...rest}
    >
      <Icon name="smile-plus" size={14} />
    </button>
  );
}
