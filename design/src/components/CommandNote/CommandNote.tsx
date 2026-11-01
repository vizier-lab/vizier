import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

export type Command = "checkpoint" | "lobotomy" | "abort";

export type CommandNoteProps = {
  /** The command the person ran, without its slash. */
  command: Command;
  /** "14:03"; when they ran it. */
  meta?: string;
} & HTMLAttributes<HTMLDivElement>;

/** A command a person ran, in the flow of the conversation (X33a).
 *
 * Emerald, not a chip: a command is a thing the person did to the conversation, so it reads as
 * a line among the messages rather than as an object attached to one. The mark is a slash,
 * because that is what they typed.
 */
export function CommandNote({ command, meta, className, children, ...rest }: CommandNoteProps) {
  return (
    <div className={cx("flex items-baseline justify-end gap-8", className)} {...rest}>
      {children}
      <span className="inline-flex items-baseline gap-2 type-mono-sm-medium text-live-text">
        <span aria-hidden className="text-live">
          /
        </span>
        {command}
      </span>
      {meta && <span className="type-mono-xs text-faint">{meta}</span>}
    </div>
  );
}
