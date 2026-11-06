import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Avatar } from "../Avatar/Avatar";
import { SkillChip } from "../SkillChip/SkillChip";

export type UserMessageProps = {
  /** Who sent it: "You", or a platform name. */
  name?: string;
  /** What the avatar's face is drawn from (the username); the name when absent. */
  seed?: string;
  /** "14:02". */
  time?: ReactNode;
  /** A photo for the avatar. */
  avatarSrc?: string;
  /** The skill it invoked: shown as a chip above the text. */
  skill?: string;
  /** Attachment chips above the text. */
  attachments?: ReactNode;
  /** Waiting behind a running turn: outlined, with when it sends. */
  queued?: { agentName: string };
  /** Right is the signed-in person (the default). Left is someone else in a Workspace, with the avatar first. */
  side?: "right" | "left";
  /** It names the signed-in person: a lapis edge on the bubble. */
  mentioned?: boolean;
} & HTMLAttributes<HTMLDivElement>;

/** A person's message, right-aligned: a meta row (time, name, avatar) above its container (D43). */
export function UserMessage({ name = "You", seed, time, avatarSrc, skill, attachments, queued, side = "right", mentioned, className, children, ...rest }: UserMessageProps) {
  const left = side === "left";
  return (
    <div className={cx("flex flex-col gap-10", left ? "items-start" : "items-end", className)} {...rest}>
      <div className="flex items-center gap-8">
        {left && <Avatar name={seed ?? name} src={avatarSrc} size="xs" square />}
        {left ? (
          <>
            <span className="type-body-strong text-fg">{name}</span>
            <span className="type-mono-xs text-faint">{time}</span>
          </>
        ) : (
          <>
            <span className="type-mono-xs text-faint">{queued ? `Queued · sends when ${queued.agentName} finishes` : time}</span>
            <span className="type-body-strong text-fg">{name}</span>
          </>
        )}
        {!left && <Avatar name={seed ?? name} src={avatarSrc} size="xs" square />}
      </div>
      <div
        className={cx(
          "flex max-w-480 min-w-0 flex-col items-start gap-6 rounded-lg px-14 py-10 [overflow-wrap:anywhere]",
          queued ? "border border-dashed border-border-strong" : "bg-surface",
          mentioned && "border-l-2 border-l-rank",
        )}
      >
        {skill && <SkillChip skill={skill} />}
        {attachments && <div className="flex w-full flex-col gap-6">{attachments}</div>}
        <div className={cx("type-prose min-w-0 max-w-full", queued ? "text-muted" : "text-fg")}>{children}</div>
      </div>
    </div>
  );
}
