import type { HTMLAttributes, ReactNode } from "react";
import { Sigil } from "../../brand/Sigil/Sigil";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { IconButton } from "../IconButton/IconButton";
import { Status, type AgentState, agentStatusLabel } from "../Status/Status";

export type MobileTopBarAgent = {
  agentId: string;
  name: string;
  state: AgentState;
  /** After the state's words: "replying", "retry in 40s". */
  detail?: ReactNode;
  /** A picture the agent was given, drawn in place of its sigil. */
  imageSrc?: string;
  /** The agent's name opens the switcher. */
  onSwitch: () => void;
  /** The button on its side opens the agent's details. Omit it to hide the button. */
  onDetails?: () => void;
};

export type MobileTopBarProps = {
  /** The ☰ that opens the menu drawer. */
  onMenu?: () => void;
  /** A page inside an agent: a back button takes the ☰'s place. */
  onBack?: () => void;
  /** The back button's name, e.g. "Back to Archivist". */
  backLabel?: string;
  /** Chat: the agent, which switches agent from its name and opens its details from the side. */
  agent?: MobileTopBarAgent;
  /** Home and every other page: a title, with an optional line under it (the agent's name). */
  title?: ReactNode;
  subtitle?: ReactNode;
  /** A second row under the bar: the conversation, which opens the topic picker, and its meta. */
  topic?: {
    title: ReactNode;
    /** "webui · new topic". */
    meta?: ReactNode;
    onTopic: () => void;
    /** What acts on this conversation (new, delete), at the end of its row, never in the bar above. */
    actions?: ReactNode;
  };
  /** The right slot: the bell, a + or the jobs count. */
  trailing?: ReactNode;
} & Omit<HTMLAttributes<HTMLElement>, "title">;

/**
 * The phone header (X75, redrawn for spec 056). No control carries a border: the ☰ or back,
 * the agent, its details button and the bell are bare, each a 44px target. The bar pads by
 * the top safe area, so it can sit under a notch.
 */
export function MobileTopBar({
  onMenu,
  onBack,
  backLabel = "Back",
  agent,
  title,
  subtitle,
  topic,
  trailing,
  className,
  ...rest
}: MobileTopBarProps) {
  const live = agent ? agent.state === "live" || agent.state === "running" : false;
  return (
    <header className={cx("flex shrink-0 flex-col bg-ground pt-safe", topic && "border-b border-border", className)} {...rest}>
      <div className="flex h-56 items-center gap-4 px-8">
        {onBack ? (
          <IconButton variant="ghost" size="lg" icon="chevron-left" aria-label={backLabel} onClick={onBack} />
        ) : onMenu ? (
          <IconButton variant="ghost" size="lg" icon="menu" aria-label="Open menu" onClick={onMenu} />
        ) : null}
        {agent ? (
          <>
            <button
              type="button"
              onClick={agent.onSwitch}
              aria-label={`${agent.name}, switch agent`}
              aria-haspopup="dialog"
              className="flex h-44 min-w-0 flex-1 items-center gap-10 rounded-lg px-4 text-left enabled:active:bg-pressed"
            >
              <Sigil agentId={agent.agentId} image={agent.imageSrc} live={live} />
              <span className="flex min-w-0 flex-col gap-2">
                <span className="flex min-w-0 items-center gap-6">
                  <span className="truncate type-body-strong text-fg">{agent.name}</span>
                  <Icon name="chevron-down" size={14} tone="muted" />
                </span>
                <Status
                  state={agent.state}
                  font="mono-xs"
                  dot={false}
                  label={agentStatusLabel(agent.state)}
                  detail={agent.detail}
                  className="min-w-0 truncate"
                />
              </span>
            </button>
            {agent.onDetails && (
              <IconButton variant="ghost" size="lg" icon="sliders" aria-label={`${agent.name} details`} onClick={agent.onDetails} />
            )}
          </>
        ) : (
          <span className="flex min-w-0 flex-1 flex-col gap-2 px-4">
            <span className="truncate type-heading text-fg">{title}</span>
            {subtitle != null && <span className="truncate type-small text-muted">{subtitle}</span>}
          </span>
        )}
        {trailing}
      </div>
      {topic && (
        <div className="flex h-44 items-center gap-12 px-16">
          <button
            type="button"
            onClick={topic.onTopic}
            aria-haspopup="dialog"
            className="flex min-w-0 items-center gap-6 rounded-sm text-left"
          >
            <span className="truncate type-ui-medium text-fg">{topic.title}</span>
            <Icon name="chevron-down" size={13} tone="muted" />
          </button>
          <span className="flex-1" />
          {topic.meta != null && <span className="min-w-0 truncate type-mono-xs text-faint">{topic.meta}</span>}
          {topic.actions != null && <span className="-mr-8 flex shrink-0 items-center">{topic.actions}</span>}
        </div>
      )}
    </header>
  );
}
