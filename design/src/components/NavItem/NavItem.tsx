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
  /** Unread notifications: a small mono pill, or a dot in the rail. Hidden at zero. */
  count?: number;
  /** Setup activity, separate from a count: a pulsing dot, not a notification. */
  busy?: boolean;
  href?: string;
  onSelect?: () => void;
} & Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "onSelect">;

/** One sidebar destination (D2). Renders a link with `href`, else a button. */
export function NavItem({ icon, label, active, collapsed, live, count, busy, href, onSelect, className, ...rest }: NavItemProps) {
  const classes = cx(
    "group flex h-31 items-center gap-10 rounded-md text-left transition-colors",
    collapsed ? "w-44 justify-center" : "w-full px-10",
    active ? "bg-raised text-fg type-body-medium" : "text-muted type-body hover:bg-surface hover:text-soft",
    className,
  );
  // The count is visual; the accessible name carries it in words (below), so a
  // screen reader hears "Notifications, 3 unread" rather than an unexplained 3.
  const unread = count != null && count > 0 ? count : 0;
  const content = (
    <>
      <Icon name={icon} className={active ? "text-fg" : "group-hover:text-fg"} />
      <span className={collapsed ? "sr-only" : "min-w-0 flex-1 truncate"}>{label}</span>
      {unread > 0 &&
        (collapsed ? (
          <span aria-hidden className="absolute top-3 right-3 size-7 rounded-pill bg-live" />
        ) : (
          <span aria-hidden className="shrink-0 rounded-pill bg-live px-5 type-mono-xs text-ground">
            {unread > 99 ? "99+" : unread}
          </span>
        ))}
      {busy && (
        <span
          aria-hidden
          className={cx(
            "size-6 shrink-0 rounded-pill bg-live animate-dot-wave motion-reduce:animate-none",
            collapsed && "absolute right-3 bottom-3",
          )}
        />
      )}
      {(unread > 0 || busy) && (
        <span className="sr-only">
          {unread > 0 ? `, ${unread} unread` : ""}
          {busy ? ", setting up" : ""}
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
