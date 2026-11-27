import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Avatar } from "../Avatar/Avatar";
import { Icon } from "../../icons/Icon";
import { SkillChip } from "../SkillChip/SkillChip";

export type UserMessageProps = {
  /** Who sent it: "You", or a platform name. */
  name?: string;
  /** What the avatar's face is drawn from (the username); the name when absent. */
  seed?: string;
  /**
   * In a Discord or Telegram conversation: whether the sender's account is connected to a Vizier
   * user (the name is then theirs) or not (the name is the platform's, which proves nothing).
   */
  sender?: "connected" | "unregistered";
  /** "14:02". */
  time?: ReactNode;
  /** A photo for the avatar. */
  avatarSrc?: string;
  /** The skill it invoked: shown as a chip above the text. */
  skill?: string;
  /** Attachment chips above the text. */
  attachments?: ReactNode;
  /**
   * Sent while the agent works: outlined, with what happens to it. `steer` (the default) goes into
   * the turn in progress at its next step; `queue` runs as a new turn after it.
   */
  queued?: { agentName: string; mode?: "steer" | "queue" };
  /** Right is the signed-in person (the default). Left is someone else in a Workspace, with the avatar first. */
  side?: "right" | "left";
  /** It names the signed-in person: a lapis edge on the bubble. */
  mentioned?: boolean;
} & HTMLAttributes<HTMLDivElement>;

function queuedLabel(queued: NonNullable<UserMessageProps["queued"]>) {
  return queued.mode === "queue" ? "Queued · runs after this turn" : "Waiting for the agent";
}

/** A person's message, right-aligned: a meta row (time, name, avatar) above its container (D43). */
export function UserMessage({ name = "You", seed, sender, time, avatarSrc, skill, attachments, queued, side = "right", mentioned, className, children, ...rest }: UserMessageProps) {
  const left = side === "left";
  const tag = sender === "connected" ? (
    <span className="flex items-center gap-4 type-mono-xs text-live-text">
      <Icon name="check" size={11} />
      connected
    </span>
  ) : sender === "unregistered" ? (
    <span className="type-mono-xs text-faint">unregistered</span>
  ) : null;
  return (
    <div className={cx("flex flex-col gap-10", left ? "items-start" : "items-end", className)} {...rest}>
      <div className="flex items-center gap-8">
        {left && <Avatar name={seed ?? name} src={avatarSrc} size="xs" square />}
        {left ? (
          <>
            <span className="type-body-strong text-fg">{name}</span>
            {tag}
            <span className="type-mono-xs text-faint">{time}</span>
          </>
        ) : (
          <>
            <span className="type-mono-xs text-faint">{queued ? queuedLabel(queued) : time}</span>
            {tag}
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
