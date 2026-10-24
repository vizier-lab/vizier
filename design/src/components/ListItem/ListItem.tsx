import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { MiddleTruncate } from "../../lib/MiddleTruncate";

export type ListItemProps = {
  /** Icon, Sigil, Avatar, ProviderLogo or a Status dot. */
  leading?: ReactNode;
  title: ReactNode;
  /** Skill names, roles, servers, paths. Long mono titles are cut in the middle. */
  titleMono?: boolean;
  /** Characters a mono title keeps before it is cut in the middle (default 40). */
  titleMax?: number;
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
  /** Rounded rows sit in pickers and panels; flush rows fill a divided list edge to edge; bare rows have no inset. */
  shape?: "rounded" | "flush" | "bare";
  /** Start-align for multi-line subtitles (activity); centre otherwise. */
  align?: "center" | "start";
} & Omit<HTMLAttributes<HTMLElement>, "title" | "onSelect">;

/**
 * The row primitive (X24) behind file, activity, revision, run, memory, agent,
 * provider and role rows, and picker rows.
 */
export function ListItem({
  leading,
  title,
  titleMono,
  titleMax = 40,
  subtitle,
  meta,
  trailing,
  selected,
  disabled,
  href,
  onSelect,
  shape = "rounded",
  align = "center",
  className,
  ...rest
}: ListItemProps) {
  const interactive = href !== undefined || onSelect !== undefined;
  const classes = cx(
    "flex w-full min-w-0 gap-10 text-left transition-colors",
    align === "center" ? "items-center" : "items-start",
    shape === "rounded" ? "rounded-md px-12 py-6" : shape === "flush" ? "px-16 py-10" : "py-2",
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
        <span className={cx("truncate text-fg", titleMono ? "type-code-medium" : "type-ui-medium")}>
          {/* Mono titles are names and paths: cut the middle, so the file name stays. */}
          {titleMono && typeof title === "string" ? <MiddleTruncate text={title} max={titleMax} /> : title}
        </span>
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
