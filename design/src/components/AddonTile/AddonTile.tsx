import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { IntegrationLogo } from "../IntegrationLogo/IntegrationLogo";

export type AddonTileProps = {
  /** Integration kind id, for the logo. */
  kind: string;
  name: string;
  description: ReactNode;
  /** "Installed", "Needs setup", "Chosen": a short state beside the name. */
  badge?: ReactNode;
  /** Chosen in a picker: the tile takes the live border and a tint. */
  selected?: boolean;
  /** Controls at the foot of the tile: Install, a switch, a checkbox. */
  children?: ReactNode;
} & HTMLAttributes<HTMLDivElement>;

/**
 * One addon in the marketplace and in the creation wizard (the addons redesign): a large logo, its name
 * and what it adds, and its controls. The tile itself is not a control; its buttons are.
 */
export function AddonTile({ kind, name, description, badge, selected = false, children, className, ...rest }: AddonTileProps) {
  return (
    <div
      className={cx(
        "flex min-h-156 flex-col gap-14 rounded-xl border bg-surface p-16 transition-colors",
        selected ? "border-live" : "border-border hover:border-border-hover",
        className,
      )}
      {...rest}
    >
      <div className="flex items-start gap-12">
        <IntegrationLogo kind={kind} size={52} />
        <div className="flex min-w-0 flex-1 flex-col gap-4">
          <div className="flex min-w-0 items-center gap-8">
            <h3 className="truncate type-heading text-fg">{name}</h3>
            {badge}
          </div>
          <p className="line-clamp-4 type-small text-muted">{description}</p>
        </div>
      </div>
      {children && <div className="mt-auto flex flex-wrap items-center justify-between gap-10">{children}</div>}
    </div>
  );
}
