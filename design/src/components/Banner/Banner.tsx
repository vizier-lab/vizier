import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type BannerTone = "offline" | "reconnected" | "failed";

export type BannerProps = {
  tone: BannerTone;
  /** The condition, one line: "Connection lost · reconnecting in 3 s". */
  title: ReactNode;
  /** A second line (failed only). */
  message?: ReactNode;
  /** Buttons: "Retry now", "Open config". */
  actions?: ReactNode;
} & Omit<HTMLAttributes<HTMLDivElement>, "title">;

/**
 * A persistent page-level condition (D26). Reconnected shows for 4 s, then
 * the caller removes it. Also used inline in chat for a lost connection.
 */
export function Banner({ tone, title, message, actions, className, ...rest }: BannerProps) {
  if (tone === "failed") {
    return (
      <div
        role="alert"
        className={cx("flex items-center gap-14 rounded-lg border border-failed-line bg-failed-bg py-14 pr-14 pl-16", className)}
        {...rest}
      >
        <Icon name="alert" tone="failed" />
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <p className="type-body-medium text-failed-text">{title}</p>
          {message && <p className="type-ui text-muted">{message}</p>}
        </div>
        {actions && <div className="flex shrink-0 items-center gap-10">{actions}</div>}
      </div>
    );
  }
  const offline = tone === "offline";
  return (
    <div
      role="status"
      className={cx(
        "flex items-center gap-8 rounded-md border px-12 py-8",
        offline ? "border-retrying-line bg-retrying-bg" : "border-border-strong bg-surface",
        className,
      )}
      {...rest}
    >
      <span aria-hidden className={cx("size-7 shrink-0 rounded-pill", offline ? "bg-retrying" : "bg-live")} />
      <p className="min-w-0 flex-1 type-small-medium text-fg">{title}</p>
      {actions && <div className="flex shrink-0 items-center gap-8">{actions}</div>}
    </div>
  );
}
