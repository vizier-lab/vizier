import type { ReactNode } from "react";
import { Sigil } from "../../brand/Sigil/Sigil";
import { Icon } from "../../icons/Icon";
import { Status, type AgentState, agentStatusLabel } from "../Status/Status";
import { ListItem, type ListItemProps } from "../ListItem/ListItem";

export type AgentRowProps = {
  agentId: string;
  name: string;
  state: AgentState;
  /** "replying", "2h ago", "retry in 40s". */
  detail?: ReactNode;
} & Omit<ListItemProps, "title" | "subtitle" | "leading" | "trailing">;

/** An agent in the mobile list (X30): sigil, name, status, chevron. Built on ListItem. */
export function AgentRow({ agentId, name, state, detail, ...rest }: AgentRowProps) {
  return (
    <ListItem
      shape="flush"
      leading={<Sigil agentId={agentId} live={state === "live" || state === "running"} />}
      title={<span className="type-body-strong">{name}</span>}
      subtitle={<Status state={state} label={agentStatusLabel(state)} detail={detail} font="mono-xs" />}
      trailing={<Icon name="chevron-right" size={15} tone="faint" />}
      {...rest}
    />
  );
}
