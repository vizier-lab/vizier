import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import type { ToneMode } from "../../tokens/tokens";
import { cx } from "../../lib/cx";

export type ToastTone = "success" | "error" | "warning" | "info";

export type ToastProps = {
  tone?: ToastTone;
  title: ReactNode;
  message?: ReactNode;
  onClose?: () => void;
} & Omit<HTMLAttributes<HTMLDivElement>, "title">;

const TONES: Record<ToastTone, { edge: string; title: string; icon: IconName; tone: ToneMode }> = {
  success: { edge: "border-border-strong", title: "text-fg", icon: "check", tone: "default" },
  error: { edge: "border-failed-line", title: "text-failed-text", icon: "alert", tone: "failed" },
  warning: { edge: "border-retrying-line", title: "text-fg", icon: "alert", tone: "retrying" },
  info: { edge: "border-border-strong", title: "text-fg", icon: "info", tone: "muted" },
};

/** A transient confirmation or failure, with a close button (D27). */
export function Toast({ tone = "success", title, message, onClose, className, ...rest }: ToastProps) {
  const t = TONES[tone];
  return (
    <div
      role={tone === "error" ? "alert" : "status"}
      className={cx("flex w-380 items-start gap-12 rounded-lg border bg-raised px-14 py-12 shadow-popover", t.edge, className)}
      {...rest}
    >
      <Icon name={t.icon} tone={t.tone} />
      <div className="flex min-w-0 flex-1 flex-col gap-2">
        <p className={cx("type-ui-medium", t.title)}>{title}</p>
        {message && <p className="type-small text-muted">{message}</p>}
      </div>
      {onClose && (
        <button type="button" aria-label="Dismiss" onClick={onClose} className="flex size-18 items-center justify-center rounded-xs text-faint hover:text-fg">
          <Icon name="close" size={13} />
        </button>
      )}
    </div>
  );
}

/** Where toasts stack: bottom right, announced politely. */
export function ToastRegion({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div aria-live="polite" className={cx("fixed right-24 bottom-24 z-50 flex flex-col gap-10", className)} {...rest} />;
}
