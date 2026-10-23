import type { HTMLAttributes, ReactNode } from "react";
import { Sigil } from "../../brand/Sigil/Sigil";
import { cx } from "../../lib/cx";

export type AgentHeaderProps = {
  agentId: string;
  name: string;
  /** "14:02". */
  time?: ReactNode;
} & HTMLAttributes<HTMLDivElement>;

/** The head of an agent's turn: sigil, name, time (D44). */
export function AgentHeader({ agentId, name, time, className, ...rest }: AgentHeaderProps) {
  return (
    <div className={cx("flex items-center gap-8", className)} {...rest}>
      <Sigil agentId={agentId} size="sm" />
      <span className="type-body-strong text-fg">{name}</span>
      {time && <span className="type-mono-xs text-faint">{time}</span>}
    </div>
  );
}
