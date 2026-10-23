import type { HTMLAttributes, ReactNode } from "react";
import { Wordmark } from "../../brand/Wordmark/Wordmark";
import { Sigil } from "../../brand/Sigil/Sigil";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { IconButton } from "../IconButton/IconButton";
import { Status, type StatusState } from "../Status/Status";

export type MobileTopBarProps = {
  onMenu: () => void;
  /** Home: the wordmark. Chat: the agent's sigil, the topic and its status. */
  chat?: {
    agentId: string;
    agentName: string;
    topic: string;
    state: Extract<StatusState, "live" | "idle" | "failed" | "retrying" | "starting" | "stopped">;
    detail?: ReactNode;
    onTopic: () => void;
  };
  /** The right slot: a + button on Home, the jobs count in chat. */
  trailing?: ReactNode;
} & HTMLAttributes<HTMLElement>;

/** The mobile header (X75). */
export function MobileTopBar({ onMenu, chat, trailing, className, ...rest }: MobileTopBarProps) {
  return (
    <header className={cx("flex h-56 items-center gap-12 border-b border-border bg-ground px-16", className)} {...rest}>
      <IconButton variant="ghost" icon="menu" aria-label="Open menu" onClick={onMenu} />
      {chat ? (
        <button type="button" onClick={chat.onTopic} className="flex min-w-0 flex-1 items-center gap-10 rounded-sm text-left">
          <Sigil agentId={chat.agentId} live={chat.state === "live"} />
          <span className="flex min-w-0 flex-col gap-2">
            <span className="flex items-center gap-4">
              <span className="truncate type-body-strong text-fg">{chat.topic}</span>
              <Icon name="chevron-down" size={13} tone="muted" />
            </span>
            <Status state={chat.state} font="mono-xs" label={chat.agentName} detail={chat.detail ?? chat.state} />
          </span>
        </button>
      ) : (
        <span className="flex flex-1 justify-center">
          <Wordmark size={22} />
        </span>
      )}
      {trailing}
    </header>
  );
}
