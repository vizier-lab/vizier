import type { HTMLAttributes, ReactNode } from "react";
import { Sigil } from "../../brand/Sigil/Sigil";
import { cx } from "../../lib/cx";
import { IconButton } from "../IconButton/IconButton";

export type NotificationItemProps = {
  /** The agent the notification is about: its sigil is drawn from this. */
  agentId: string;
  agentName: string;
  /** The conversation's or task's title. Falls back to the agent's name when there is none. */
  title?: string;
  /** What happened, in a few words. */
  text: ReactNode;
  /** A short plain-text excerpt of what was said; two lines at most. */
  excerpt?: string;
  time: ReactNode;
  /** ISO time for the `<time>` element. */
  dateTime?: string;
  read: boolean;
  tone?: "default" | "failed" | "success";
  /** The card's body was pressed. */
  onOpen?: () => void;
  /** The eye was pressed. Omit to hide the toggle. */
  onToggleRead?: () => void;
} & Omit<HTMLAttributes<HTMLDivElement>, "title" | "children">;

/**
 * One notification as a card in the notifications popup: a sigil, the conversation title, what
 * happened and a short excerpt, and a check beside the time that marks it read (faint once it is,
 * and pressing it again marks it unread). An unread card is outlined in the live emerald. The
 * check is always drawn, so it is as reachable on a phone as with a mouse.
 */
export function NotificationItem({
  agentId,
  agentName,
  title,
  text,
  excerpt,
  time,
  dateTime,
  read,
  tone = "default",
  onOpen,
  onToggleRead,
  className,
  ...rest
}: NotificationItemProps) {
  const textTone = tone === "failed" ? "text-failed-text" : tone === "success" ? "text-fg" : read ? "text-faint" : "text-muted";
  return (
    <div
      className={cx(
        "flex w-full min-w-0 items-start gap-12 rounded-lg border px-12 py-12 transition-colors",
        read ? "border-border bg-transparent hover:border-border-strong" : "border-live bg-raised",
        className,
      )}
      {...rest}
    >
      <Sigil agentId={agentId} size="md" label={agentName} />
      <button type="button" onClick={onOpen} className="flex min-w-0 flex-1 flex-col gap-3 text-left">
        <span className={cx("min-w-0 truncate", read ? "type-ui-medium text-muted" : "type-ui-strong text-fg")}>{title ?? agentName}</span>
        <span className={cx("type-small", textTone)}>
          {text}
          {!read && <span className="sr-only"> Unread</span>}
        </span>
        {excerpt && <span className="line-clamp-2 type-small text-faint">{excerpt}</span>}
      </button>
      <div className="flex shrink-0 items-center gap-4">
        <time dateTime={dateTime} className="type-small-medium whitespace-nowrap text-faint">
          {time}
        </time>
        {onToggleRead && (
          <IconButton
            variant="ghost"
            size="sm"
            icon="check"
            aria-label={read ? "Mark as unread" : "Mark as read"}
            className={read ? "text-faint" : undefined}
            onClick={onToggleRead}
          />
        )}
      </div>
    </div>
  );
}
