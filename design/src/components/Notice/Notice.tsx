import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import type { ToneMode } from "../../tokens/tokens";
import { cx } from "../../lib/cx";

export type NoticeTone = "failed" | "retrying" | "info";

export type NoticeProps = {
  tone?: NoticeTone;
  title?: ReactNode;
  /** One or more actions: a Button, or TextLinks ("Retry · Switch model"). */
  actions?: ReactNode;
  /** Inside the activity trail: one mono line, no title, no ground. */
  compact?: boolean;
  /** Overrides the tone's icon, e.g. `alert` for a limit. */
  icon?: IconName;
} & Omit<HTMLAttributes<HTMLDivElement>, "title">;

const TONES: Record<NoticeTone, { box: string; title: string; icon: IconName; tone: ToneMode }> = {
  failed: { box: "border-failed-line bg-failed-bg", title: "text-failed-text", icon: "alert", tone: "failed" },
  retrying: { box: "border-retrying-line bg-retrying-bg", title: "text-retrying", icon: "refresh", tone: "retrying" },
  info: { box: "border-border-strong bg-raised", title: "text-fg", icon: "info", tone: "default" },
};

/** An inline message with tone, title, message and actions (D25). */
export function Notice({ tone = "info", title, actions, compact, icon, className, children, ...rest }: NoticeProps) {
  const t = TONES[tone];
  if (compact) {
    return (
      <div role={tone === "failed" ? "alert" : "status"} className={cx("flex items-center gap-8", className)} {...rest}>
        <Icon name={icon ?? t.icon} size={14} tone={t.tone} />
        <span className={cx("min-w-0 flex-1 type-mono-sm", t.title)}>{children}</span>
        {actions && <span className="flex shrink-0 items-center gap-14">{actions}</span>}
      </div>
    );
  }
  return (
    <div
      role={tone === "failed" ? "alert" : "status"}
      className={cx("flex items-start gap-12 rounded-lg border px-14 py-12", t.box, className)}
      {...rest}
    >
      <Icon name={icon ?? t.icon} tone={t.tone} className="mt-px" />
      <div className="flex min-w-0 flex-1 flex-col gap-2">
        {title && <p className={cx("type-ui-strong", t.title)}>{title}</p>}
        {children && <div className="type-small-para text-muted">{children}</div>}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-14">{actions}</div>}
    </div>
  );
}
