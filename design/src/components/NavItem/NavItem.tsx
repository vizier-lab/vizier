import type { AnchorHTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";

export type NavItemProps = {
  icon: IconName;
  label: ReactNode;
  /** The current page. Sets aria-current. */
  active?: boolean;
  /** The 64px rail: icon only, the label becomes the accessible name. */
  collapsed?: boolean;
  /** An emerald ring at the end, e.g. Chat while a reply is streaming. */
  live?: boolean;
  /** Shown but not reachable, e.g. an agent page while no agent is chosen. */
  disabled?: boolean;
  href?: string;
  onSelect?: () => void;
} & Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "onSelect">;

/** One sidebar destination (D2). Renders a link with `href`, else a button. */
export function NavItem({ icon, label, active, collapsed, live, disabled, href, onSelect, className, ...rest }: NavItemProps) {
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
      <Icon name={icon} className={disabled ? undefined : active ? "text-fg" : "group-hover:text-fg"} />
      <span className={collapsed ? "sr-only" : "min-w-0 flex-1 truncate"}>{label}</span>
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
