import { Icon } from "../../icons/Icon";
import { Badge } from "../Badge/Badge";
import { ListItem, type ListItemProps } from "../ListItem/ListItem";

export type RoleRowProps = {
  name: string;
  builtIn?: boolean;
  /** Granted permissions; "all permissions" when `all`. */
  permissions: number | "all";
  people: number;
} & Omit<ListItemProps, "title" | "subtitle" | "meta" | "leading" | "trailing">;

/** A role in the roles list (X67): name, built-in badge, permission count, people. Built on ListItem. */
export function RoleRow({ name, builtIn, permissions, people, ...rest }: RoleRowProps) {
  const perms = permissions === "all" ? "all permissions" : `${permissions} permission${permissions === 1 ? "" : "s"}`;
  return (
    <ListItem
      shape="flush"
      title={
        <span className="flex items-center gap-8">
          <span className="truncate type-code-medium text-fg">{name}</span>
          {builtIn && <Badge tone="rank">built-in</Badge>}
        </span>
      }
      subtitle={builtIn ? perms : `custom · ${perms}`}
      meta={
        <span className="inline-flex items-center gap-4" aria-label={`${people} people`}>
          <Icon name="user" size={13} tone="muted" />
          {people}
        </span>
      }
      {...rest}
    />
  );
}
