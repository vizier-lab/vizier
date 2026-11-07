import type { HTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import { Badge } from "../Badge/Badge";

export type OriginKind = "task" | "chat" | "discord" | "telegram" | "workspace";

const ICONS: Record<OriginKind, IconName> = {
  task: "calendar",
  chat: "chat",
  discord: "channels",
  telegram: "channels",
  workspace: "users",
};

export type OriginBadgeProps = {
  kind: OriginKind;
  /** What it came from: a task's name, "Discord", another Workspace's name. */
  label: string;
} & HTMLAttributes<HTMLSpanElement>;

/** Where a message really came from, when not from inside the topic (X62): "from Discord", "from task daily-report". */
export function OriginBadge({ kind, label, ...rest }: OriginBadgeProps) {
  return (
    <Badge {...rest}>
      <Icon name={ICONS[kind]} size={11} aria-hidden className="mr-2" />
      from {kind === "task" ? `task ${label}` : label}
    </Badge>
  );
}
