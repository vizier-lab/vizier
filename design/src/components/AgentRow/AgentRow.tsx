import type { ReactNode } from "react";
import { Sigil } from "../../brand/Sigil/Sigil";
import { Icon } from "../../icons/Icon";
import { IconButton } from "../IconButton/IconButton";
import { Status, type AgentState, agentStatusLabel } from "../Status/Status";
import { ListItem, type ListItemProps } from "../ListItem/ListItem";

export type AgentRowProps = {
  agentId: string;
  name: string;
  state: AgentState;
  /** "replying", "2h ago", "retry in 40s". */
  detail?: ReactNode;
  /** A picture the agent was given, drawn in place of its sigil. */
  imageSrc?: string;
  /**
   * A details button beside the row (the phone's agent switcher, spec 056): it opens the
   * agent's details without choosing the agent. It sits beside the row's own link or button,
   * never inside it.
   */
  onDetails?: () => void;
  /** The chevron at the end. Off in the switcher, where the details button ends the row. */
  chevron?: boolean;
} & Omit<ListItemProps, "title" | "subtitle" | "leading" | "trailing" | "action">;

/** An agent in the mobile list (X30): sigil, name, status, chevron. Built on ListItem. */
export function AgentRow({ agentId, name, state, detail, imageSrc, onDetails, chevron = true, ...rest }: AgentRowProps) {
  return (
    <ListItem
      shape="flush"
      leading={<Sigil agentId={agentId} image={imageSrc} live={state === "live" || state === "running"} />}
      title={<span className="type-body-strong">{name}</span>}
      subtitle={<Status state={state} label={agentStatusLabel(state)} detail={detail} font="mono-xs" />}
      trailing={chevron ? <Icon name="chevron-right" size={15} tone="faint" /> : undefined}
      action={
        onDetails ? (
          <IconButton variant="ghost" size="lg" icon="sliders" aria-label={`${name} details`} onClick={onDetails} />
        ) : undefined
      }
      {...rest}
    />
  );
}
