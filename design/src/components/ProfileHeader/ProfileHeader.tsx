import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Avatar } from "../Avatar/Avatar";
import { Badge } from "../Badge/Badge";

export type ProfileHeaderProps = {
  name: string;
  role?: ReactNode;
  avatarSrc?: string;
} & HTMLAttributes<HTMLDivElement>;

/** The signed-in person at the top of Profile (X66). */
export function ProfileHeader({ name, role, avatarSrc, className, ...rest }: ProfileHeaderProps) {
  return (
    <div className={cx("flex items-center gap-16 rounded-xl border border-border bg-surface px-24 py-20", className)} {...rest}>
      <Avatar name={name} src={avatarSrc} size="lg" />
      <div className="flex flex-col items-start gap-4">
        <h2 className="type-title text-fg">{name}</h2>
        {role && <Badge tone="rank">{role}</Badge>}
      </div>
    </div>
  );
}
