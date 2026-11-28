import type { AnchorHTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";

export type NavItemProps = {
  /** Absent for a row that is a name only, such as a topic (`# general`). */
  icon?: IconName;
  label: ReactNode;
  /** The current page. Sets aria-current. */
  active?: boolean;
  /** The 64px rail: icon only, the label becomes the accessible name. */
  collapsed?: boolean;
  /** An emerald ring at the end, e.g. Chat while a reply is streaming. */
  live?: boolean;
  /**
   * How many things at the end wait for the person, in the waiting colour. Nothing is drawn for
   * 0. In the collapsed rail it is left out; the name carries it.
   */
  count?: number;
  /** What the count means, for assistive tech: "2 need you". */
  countLabel?: string;
  /** Shown but not reachable, e.g. an agent page while no agent is chosen. */
  disabled?: boolean;
  href?: string;
  onSelect?: () => void;
} & Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "onSelect">;

/** One sidebar destination (D2). Renders a link with `href`, else a button. */
export function NavItem({ icon, label, active, collapsed, live, count, countLabel, disabled, href, onSelect, className, ...rest }: NavItemProps) {
  const classes = cx(
    "group flex h-31 items-center gap-10 rounded-md text-left transition-colors",
    collapsed ? "w-44 justify-center" : "w-full px-10",
    disabled
      ? "cursor-not-allowed text-faint type-body"
      : active
        ? "bg-raised text-fg type-body-medium"
        : "text-muted type-body hover:bg-surface hover:text-soft",
    className,
  );
  const content = (
    <>
      {icon && <Icon name={icon} className={disabled ? undefined : active ? "text-fg" : "group-hover:text-fg"} />}
      <span className={collapsed ? "sr-only" : "min-w-0 flex-1 truncate"}>{label}</span>
      {!!count && !collapsed && (
        <span className="flex h-16 min-w-20 shrink-0 items-center justify-center rounded-pill bg-retrying-bg px-6 type-mono-xs-medium text-retrying">
          <span aria-hidden>{count}</span>
          <span className="sr-only">, {countLabel ?? `${count} waiting`}</span>
        </span>
      )}
      {live && (
        <span
          aria-label="live"
          role="img"
          className={cx("size-12 shrink-0 rounded-pill border border-live", collapsed && "absolute -right-4")}
        />
      )}
    </>
  );
  // No link and no handler: nothing to follow, but still in the tab order so its name (and the
  // rail's tooltip) can be reached.
  if (disabled) {
    return (
      <span
        role="link"
        aria-disabled="true"
        tabIndex={0}
        className={cx(classes, collapsed && "relative")}
        {...(rest as React.HTMLAttributes<HTMLSpanElement>)}
      >
        {content}
      </span>
    );
  }
  if (href !== undefined) {
    return (
      <a href={href} aria-current={active ? "page" : undefined} className={cx(classes, collapsed && "relative")} {...rest}>
        {content}
      </a>
    );
  }
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-current={active ? "page" : undefined}
      className={cx(classes, collapsed && "relative")}
      {...(rest as React.ButtonHTMLAttributes<HTMLButtonElement>)}
    >
      {content}
    </button>
  );
}
