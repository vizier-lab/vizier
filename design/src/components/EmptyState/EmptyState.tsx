import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";

export type EmptyStateProps = {
  /** Empty (with an action), No results (with a hint), Error (load failed, with a retry). */
  kind?: "empty" | "no-results" | "error";
  /** Empty states name their page's icon; the others default to search and alert. */
  icon?: IconName;
  title: ReactNode;
  message?: ReactNode;
  action?: ReactNode;
} & Omit<HTMLAttributes<HTMLDivElement>, "title">;

/** What a list shows when it has nothing to show (D29). */
export function EmptyState({ kind = "empty", icon, title, message, action, className, ...rest }: EmptyStateProps) {
  const glyph = icon ?? (kind === "error" ? "alert" : kind === "no-results" ? "search" : "circle-check");
  return (
    <div
      role={kind === "error" ? "alert" : undefined}
      className={cx("flex flex-col items-center gap-12 rounded-xl border border-dashed border-border-strong p-24 text-center", className)}
      {...rest}
    >
      <Icon name={glyph} size={22} tone={kind === "error" ? "failed" : "muted"} />
      <h3 className="type-heading text-fg">{title}</h3>
      {message && <p className="max-w-364 type-ui-para text-muted">{message}</p>}
      {action && <div className="pt-4">{action}</div>}
    </div>
  );
}
