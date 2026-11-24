import type { ReactNode } from "react";
import { ListItem, type ListItemProps } from "../ListItem/ListItem";

export type RevisionRowProps = {
  revision: number;
  /** "ganesha · WebUI", "dream cycle", "Archivist · write_core". */
  author: ReactNode;
  time: ReactNode;
  /** Lines added and removed against the revision before; omit when the list doesn't know them. */
  added?: number;
  removed?: number;
} & Omit<ListItemProps, "title" | "subtitle" | "meta" | "leading">;

/** One revision in a history (X27): rev N, who and how, when, +n −m. Built on ListItem. */
export function RevisionRow({ revision, author, time, added, removed, ...rest }: RevisionRowProps) {
  return (
    <ListItem
      shape="flush"
      leading={<span className="w-40 type-mono-sm text-fg">rev {revision}</span>}
      title={author}
      subtitle={<span className="type-mono-xs text-faint">{time}</span>}
      meta={
        added === undefined || removed === undefined ? undefined : (
          <span aria-label={`${added} lines added, ${removed} removed`}>
            +{added} −{removed}
          </span>
        )
      }
      {...rest}
    />
  );
}
