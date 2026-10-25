import type { HTMLAttributes, ReactNode } from "react";
import { motion, type HTMLMotionProps } from "motion/react";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import type { ToneMode } from "../../tokens/tokens";
import { cx } from "../../lib/cx";
import { motionDurations, motionEases } from "../../tokens/tokens";

export type ToastTone = "success" | "error" | "warning" | "info";

export type ToastProps = {
  tone?: ToastTone;
  title: ReactNode;
  message?: ReactNode;
  /** Makes the toast's content a control: click it to open what it is about. */
  onSelect?: () => void;
  onClose?: () => void;
} & Omit<HTMLAttributes<HTMLDivElement>, "title">;

const TONES: Record<ToastTone, { edge: string; title: string; icon: IconName; tone: ToneMode }> = {
  success: { edge: "border-border-strong", title: "text-fg", icon: "check", tone: "default" },
  error: { edge: "border-failed-line", title: "text-failed-text", icon: "alert", tone: "failed" },
  warning: { edge: "border-retrying-line", title: "text-fg", icon: "alert", tone: "retrying" },
  info: { edge: "border-border-strong", title: "text-fg", icon: "info", tone: "muted" },
};

/**
 * A transient confirmation or failure, with a close button (D27).
 *
 * The toast carries its own entrance and exit, and `layout` so the ones below
 * it glide up when one is dismissed. Wrap the toasts in `Presence` so the
 * exit can play.
 */
export function Toast({ tone = "success", title, message, onSelect, onClose, className, ...rest }: ToastProps) {
  const t = TONES[tone];
  const body = (
    <>
      <Icon name={t.icon} tone={t.tone} />
      <div className="flex min-w-0 flex-1 flex-col gap-2">
        <p className={cx("type-ui-medium", t.title)}>{title}</p>
        {message && <p className="type-small text-muted">{message}</p>}
      </div>
    </>
  );
  return (
    <motion.div
      role={tone === "error" ? "alert" : "status"}
      layout
      initial={{ opacity: 0, x: 12 }}
      animate={{
        opacity: 1,
        x: 0,
        transition: { duration: motionDurations.base, ease: motionEases.arrive },
      }}
      exit={{
        opacity: 0,
        x: 8,
        transition: { duration: motionDurations.fast, ease: motionEases.standard },
      }}
      className={cx("flex w-380 items-start gap-12 rounded-lg border bg-raised px-14 py-12 shadow-popover", t.edge, className)}
      {...(rest as HTMLMotionProps<"div">)}
    >
      {onSelect ? (
        <button
          type="button"
          onClick={onSelect}
          className="flex min-w-0 flex-1 items-start gap-12 rounded-sm text-left transition-opacity hover:opacity-85"
        >
          {body}
        </button>
      ) : (
        body
      )}
      {onClose && (
        <button
          type="button"
          aria-label="Dismiss"
          onClick={onClose}
          className="flex size-18 items-center justify-center rounded-xs text-faint transition-colors hover:text-fg"
        >
          <Icon name="close" size={13} />
        </button>
      )}
    </motion.div>
  );
}

/** Where toasts stack: bottom right, announced politely. */
export function ToastRegion({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div aria-live="polite" className={cx("fixed right-24 bottom-24 z-50 flex flex-col gap-10", className)} {...rest} />;
}
