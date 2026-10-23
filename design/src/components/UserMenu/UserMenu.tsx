import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { Avatar } from "../Avatar/Avatar";
import { Badge } from "../Badge/Badge";

export type UserMenuProps = {
  name: string;
  /** Their role, shown as a rank badge: "superadmin", "owner". */
  role?: ReactNode;
  avatarSrc?: string;
  /** The 64px rail: avatar only. */
  collapsed?: boolean;
  /** Opens the profile. */
  onOpenProfile?: () => void;
  onSignOut: () => void;
} & HTMLAttributes<HTMLDivElement>;

/** The signed-in person at the foot of the sidebar, with sign out (D5). */
export function UserMenu({ name, role, avatarSrc, collapsed, onOpenProfile, onSignOut, className, ...rest }: UserMenuProps) {
  return (
    <div
      className={cx("flex border-t border-border py-10", collapsed ? "w-full flex-col items-center gap-6" : "items-center gap-10 pr-6 pl-10", className)}
      {...rest}
    >
      <button
        type="button"
        onClick={onOpenProfile}
        aria-label={collapsed ? `${name}, profile` : undefined}
        className={cx("flex min-w-0 items-center gap-10 rounded-md text-left hover:opacity-80", collapsed ? "justify-center" : "flex-1")}
      >
        <Avatar name={name} src={avatarSrc} size="sm" />
        {!collapsed && (
          <span className="flex min-w-0 flex-col items-start gap-2">
            <span className="truncate type-ui-medium text-fg">{name}</span>
            {role && <Badge tone="rank">{role}</Badge>}
          </span>
        )}
      </button>
      {(
        <button
          type="button"
          aria-label="Sign out"
          onClick={onSignOut}
          className="flex size-28 shrink-0 items-center justify-center rounded-sm text-faint hover:bg-raised hover:text-fg"
        >
          <Icon name="logout" size={16} />
        </button>
      )}
    </div>
  );
}
