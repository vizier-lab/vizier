import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type ListItemProps = {
  /** Icon, Sigil, Avatar, ProviderLogo or a Status dot. */
  leading?: ReactNode;
  title: ReactNode;
  /** Skill names, roles, servers, paths. */
  titleMono?: boolean;
  /** Second line; plain text is muted, or pass a styled node (a status line). */
  subtitle?: ReactNode;
  /** Right-aligned mono meta: sizes, times, counts. */
  meta?: ReactNode;
  /** Badges, buttons, a chevron or ✓, after the meta. */
  trailing?: ReactNode;
  selected?: boolean;
  disabled?: boolean;
  /** A link row. */
  href?: string;
  /** A button row. Without href or onSelect the row is static. */
  onSelect?: () => void;
} & Omit<HTMLAttributes<HTMLElement>, "title" | "onSelect">;

/**
 * The row primitive (X24) behind file, activity, revision, run, memory, agent,
 * provider and role rows, and picker rows.
 */
export function ListItem({
  leading,
  title,
  titleMono,
  subtitle,
  meta,
  trailing,
  selected,
  disabled,
  href,
  onSelect,
  className,
  ...rest
}: ListItemProps) {
  const interactive = href !== undefined || onSelect !== undefined;
  const classes = cx(
    "flex w-full min-w-0 items-center gap-10 rounded-md px-12 py-6 text-left",
    selected && "bg-raised",
    interactive && !disabled && !selected && "hover:bg-raised active:bg-pressed",
    interactive && "cursor-pointer",
    disabled && "pointer-events-none opacity-40",
    className,
  );
  const body = (
    <>
      {leading && <span className="flex shrink-0 items-center">{leading}</span>}
      <span className="flex min-w-0 flex-1 flex-col gap-2">
        <span className={cx("truncate text-fg", titleMono ? "type-code-medium" : "type-ui-medium")}>{title}</span>
        {subtitle != null &&
          (typeof subtitle === "string" ? <span className="truncate type-small text-muted">{subtitle}</span> : subtitle)}
      </span>
      {meta != null && <span className="shrink-0 type-mono-xs text-faint whitespace-nowrap">{meta}</span>}
      {trailing != null && <span className="flex shrink-0 items-center gap-8">{trailing}</span>}
    </>
  );
  if (href !== undefined) {
    return (
      <a href={disabled ? undefined : href} aria-disabled={disabled || undefined} aria-current={selected ? "true" : undefined} className={classes} {...rest}>
        {body}
      </a>
    );
  }
  if (onSelect !== undefined) {
    return (
      <button
        type="button"
        disabled={disabled}
        aria-pressed={rest.role ? undefined : selected}
        aria-selected={rest.role === "option" ? (selected ?? false) : undefined}
        onClick={onSelect}
        className={classes}
        {...(rest as HTMLAttributes<HTMLButtonElement>)}
      >
        {body}
      </button>
    );
  }
  return (
    <div className={classes} {...rest}>
      {body}
    </div>
  );
}
