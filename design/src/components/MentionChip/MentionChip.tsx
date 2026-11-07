import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

export type MentionChipProps = {
  /** The handle without its "@": an agent id or a username. */
  handle: string;
  /** Tone carries meaning: resolved is lapis, you is emerald (it is for the signed-in person). */
  tone?: "resolved" | "you";
} & HTMLAttributes<HTMLSpanElement>;

/** An `@handle` inside a Workspace message (X60). It marks who a message names; it wakes nobody by itself. */
export function MentionChip({ handle, tone = "resolved", className, ...rest }: MentionChipProps) {
  return (
    <span
      className={cx(
        "inline-flex items-center rounded-sm px-4 type-code-medium",
        tone === "you" ? "bg-raised text-live-text" : "bg-rank-bg text-rank-text",
        className,
      )}
      {...rest}
    >
      @{handle.replace(/^@/, "")}
    </span>
  );
}
