import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Sigil } from "../../brand/Sigil/Sigil";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { Status, type StatusState } from "../Status/Status";

export type AgentSwitcherProps = {
  agentId: string;
  name: string;
  state: Extract<StatusState, "live" | "idle" | "failed" | "retrying" | "starting" | "stopped">;
  /** "replying", "2h ago", "retry in 40s". */
  detail?: ReactNode;
  /** The 64px rail: sigil only. */
  collapsed?: boolean;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/**
 * The current agent at the top of the sidebar (D4). It opens the agent
 * Picker: pass it as the Picker's `trigger`, which sets aria-expanded.
 */
export function AgentSwitcher({ agentId, name, state, detail, collapsed, type = "button", className, ...rest }: AgentSwitcherProps) {
  const open = rest["aria-expanded"] === true;
  return (
    <button
      type={type}
      aria-label={collapsed ? `${name}, switch agent` : undefined}
      className={cx(
        "flex items-center gap-10 rounded-lg border text-left",
        collapsed ? "size-44 justify-center p-4" : "w-full p-10",
        open ? "border-border-strong bg-raised" : "border-border bg-surface hover:bg-raised",
        className,
      )}
      {...rest}
    >
      <Sigil agentId={agentId} live={state === "live"} />
      {!collapsed && (
        <>
          <span className="flex min-w-0 flex-1 flex-col gap-2">
            <span className="truncate type-body-strong text-fg">{name}</span>
            <Status state={state} detail={detail} font="mono-xs" dot={false} />
          </span>
          <Icon name="chevron-down" size={14} tone="faint" />
        </>
      )}
    </button>
  );
}
