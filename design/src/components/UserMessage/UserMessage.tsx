import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { SkillChip } from "../SkillChip/SkillChip";

export type UserMessageProps = {
  /** "You · 14:02". */
  meta?: ReactNode;
  /** The skill it invoked: shown as a chip above the text. */
  skill?: string;
  /** Attachment chips above the text. */
  attachments?: ReactNode;
  /** Waiting behind a running turn: outlined, with when it sends. */
  queued?: { agentName: string };
} & HTMLAttributes<HTMLDivElement>;

/** A person's message, right-aligned (D43). */
export function UserMessage({ meta, skill, attachments, queued, className, children, ...rest }: UserMessageProps) {
  return (
    <div className={cx("flex flex-col items-end gap-6", className)} {...rest}>
      <div
        className={cx(
          "flex max-w-480 min-w-0 flex-col items-start gap-6 rounded-lg px-14 py-10 [overflow-wrap:anywhere]",
          queued ? "border border-dashed border-border-strong" : "bg-surface",
        )}
      >
        {skill && <SkillChip skill={skill} />}
        {attachments && <div className="flex w-full flex-col gap-6">{attachments}</div>}
        <div className={cx("type-body-para min-w-0 max-w-full", queued ? "text-muted" : "text-fg")}>{children}</div>
      </div>
      <span className="type-mono-xs text-faint">{queued ? `Queued · sends when ${queued.agentName} finishes` : meta}</span>
    </div>
  );
}
