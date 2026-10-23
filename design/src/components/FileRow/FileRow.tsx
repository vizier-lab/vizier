import type { ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { ListItem, type ListItemProps } from "../ListItem/ListItem";

export type FileRowProps = {
  name: string;
  /** "12 KB". */
  size?: ReactNode;
  /** Buttons after the size: download, remove. */
  actions?: ReactNode;
} & Omit<ListItemProps, "title" | "leading" | "meta" | "trailing">;

/** A file in a list (X25): session files, skill resources. Built on ListItem. */
export function FileRow({ name, size, actions, ...rest }: FileRowProps) {
  return <ListItem leading={<Icon name="file" size={15} tone="muted" />} title={name} titleMono meta={size} trailing={actions} {...rest} />;
}
