import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { IconButton } from "../IconButton/IconButton";

export type AgentReplyProps = {
  /** The reply, usually rendered markdown in a Prose. */
  children: ReactNode;
  /** Streaming shows a live caret; failed shows why under the text. */
  state?: "default" | "streaming" | "failed";
  /** Failed: "Stopped: the provider didn't answer in 120 s. What arrived is kept; send again to retry." */
  error?: ReactNode;
  /** Reactions and the add-reaction button. */
  reactions?: ReactNode;
  /** "anthropic/claude-sonnet-5-5 · 6.1k in · 712 out · ~$0.03". */
  stats?: ReactNode;
  onCopy?: () => void;
  onMore?: () => void;
} & HTMLAttributes<HTMLDivElement>;

/** An agent's reply with its meta row (D45). Copy and ⋯ show on hover or focus. */
export function AgentReply({ children, state = "default", error, reactions, stats, onCopy, onMore, className, ...rest }: AgentReplyProps) {
  return (
    <div className={cx("group flex flex-col gap-10", className)} {...rest}>
      <div className="type-prose text-fg" aria-busy={state === "streaming" || undefined}>
        {children}
        {state === "streaming" && (
          <span aria-hidden className="ml-2 inline-block h-18 w-2 translate-y-3 bg-live animate-shimmer motion-reduce:animate-none" />
        )}
      </div>
      {state === "failed" && error && (
        <p role="alert" className="flex items-start gap-6 type-small text-failed-text">
          <Icon name="alert" size={14} tone="failed" className="mt-2" />
          {error}
        </p>
      )}
      {state !== "streaming" && (reactions || stats || onCopy || onMore) && (
        <div className="flex min-h-34 items-center gap-12">
          {reactions && <div className="flex items-center gap-6">{reactions}</div>}
          {stats && <span className="min-w-0 truncate type-mono-xs text-faint">{stats}</span>}
          <div className="ml-auto flex items-center gap-2 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
            {onCopy && <IconButton variant="ghost" size="sm" icon="copy" aria-label="Copy reply" onClick={onCopy} />}
            {onMore && <IconButton variant="ghost" size="sm" icon="more" aria-label="More actions" onClick={onMore} />}
          </div>
        </div>
      )}
    </div>
  );
}
