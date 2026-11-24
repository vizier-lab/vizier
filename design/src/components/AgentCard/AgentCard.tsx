import type { HTMLAttributes, ReactNode } from "react";
import { Sigil } from "../../brand/Sigil/Sigil";
import { cx } from "../../lib/cx";
import { Badge } from "../Badge/Badge";
import { Status, type AgentState, agentStatusLabel } from "../Status/Status";

export type AgentCardProps = {
  agentId: string;
  name: string;
  /** "memory & research". */
  description?: ReactNode;
  state: AgentState;
  detail?: ReactNode;
  /** "anthropic/claude-sonnet-5-5". */
  model?: string;
  owned?: boolean;
  href: string;
} & HTMLAttributes<HTMLAnchorElement>;

/** An agent on Home's court (X53). Failed agents get the ember edge. */
export function AgentCard({ agentId, name, description, state, detail, model, owned, href, className, ...rest }: AgentCardProps) {
  return (
    <a
      href={href}
      className={cx(
        // Cards sit in a grid, so each fills its cell to the row's height.
        "flex h-full flex-col gap-14 rounded-lg border bg-surface px-16 py-14 transition-colors hover:bg-raised",
        state === "failed" ? "border-failed-line" : "border-border",
        className,
      )}
      {...rest}
    >
      <span className="flex items-center gap-12">
        <Sigil agentId={agentId} live={state === "live" || state === "running"} />
        <span className="flex min-w-0 flex-col gap-2">
          <span className="flex items-center gap-8">
            <span className="truncate type-body-strong text-fg">{name}</span>
            {owned && <Badge tone="rank">Owner</Badge>}
          </span>
          {description && <span className="truncate type-small text-muted">{description}</span>}
        </span>
      </span>
      <Status state={state} label={agentStatusLabel(state)} detail={detail} />
      {model && <span className="truncate type-mono-xs text-faint">{model}</span>}
    </a>
  );
}
