import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Sigil } from "../../brand/Sigil/Sigil";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { Status, type AgentState, agentStatusLabel } from "../Status/Status";

export type SidebarAgentProps = {
  agentId: string;
  name: string;
  /** What the agent is for. Shown under the name, above the status. */
  description?: ReactNode;
  state: AgentState;
  /** "replying", "retry in 40s". */
  detail?: ReactNode;
  /** The page open lives in this card's menu (Core, Channels, Usage …), or the menu is open. */
  active?: boolean;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/**
 * The agent at the top of the context sidebar. It opens the agent's menu: pass it as the trigger of
 * the menu's popover, which sets aria-expanded. The card is lit while the menu is open or while the
 * page open is one of the menu's.
 */
export function SidebarAgent({ agentId, name, description, state, detail, active, type = "button", className, ...rest }: SidebarAgentProps) {
  const lit = active || rest["aria-expanded"] === true;
  return (
    <button
      type={type}
      className={cx(
        "flex min-h-52 w-full items-center gap-10 rounded-lg border py-8 pr-14 pl-8 text-left transition-colors duration-[var(--vz-duration-base)]",
        lit ? "border-border-strong bg-raised" : "border-border bg-surface/40 hover:bg-surface",
        className,
      )}
      {...rest}
    >
      <Sigil size="card" agentId={agentId} live={state === "live" || state === "running"} />
      <span className="flex min-w-0 flex-1 flex-col gap-2">
        <span className="truncate type-body-strong text-fg">{name}</span>
        {description && <span className="truncate type-small text-muted">{description}</span>}
        <Status state={state} label={agentStatusLabel(state)} detail={detail} font="mono-xs" dot={false} />
      </span>
      <Icon name="chevron-down" size={14} tone="faint" className={cx("transition-transform duration-[var(--vz-duration-base)]", rest["aria-expanded"] === true && "rotate-180")} />
    </button>
  );
}
