import type { ReactNode } from "react";
import { cx } from "../../lib/cx";
import { ListItem, type ListItemProps } from "../ListItem/ListItem";

export type ActivityItemProps = {
  /** Who: an agent or a person. */
  actor: ReactNode;
  /** "21:04", "yesterday". */
  time: ReactNode;
  /** What happened, a sentence. */
  text: ReactNode;
  /** A Sigil or Avatar (Home); omit for the plain integration log. */
  leading?: ReactNode;
  failed?: boolean;
} & Omit<ListItemProps, "title" | "subtitle" | "meta" | "leading">;

/** One line of activity (X26): who, when, what. Built on ListItem. */
export function ActivityItem({ actor, time, text, leading, failed, className, ...rest }: ActivityItemProps) {
  return (
    <ListItem
      leading={leading}
      title={actor}
      subtitle={<span className={cx("type-ui-para", failed ? "text-failed-text" : "text-muted")}>{text}</span>}
      meta={time}
      align="start"
      className={className}
      {...rest}
    />
  );
}
