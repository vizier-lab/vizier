import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type ProgressBarProps = {
  label: ReactNode;
  /** 0–100. Omit for indeterminate (size unknown). */
  value?: number;
  state?: "running" | "done" | "failed";
  /** Under the bar: "12.4 GB of 20.0 GB · about 3 min", or what went wrong. */
  detail?: ReactNode;
} & HTMLAttributes<HTMLDivElement>;

/** Progress with a label, value and detail (D30). */
export function ProgressBar({ label, value, state = "running", detail, className, ...rest }: ProgressBarProps) {
  const indeterminate = value === undefined && state === "running";
  const pct = state === "done" ? 100 : Math.max(0, Math.min(100, value ?? 0));
  const failed = state === "failed";
  return (
    <div className={cx("flex flex-col gap-8", className)} {...rest}>
      <div className="flex items-center gap-8">
        <span className={cx("min-w-0 flex-1 truncate type-ui-medium", failed ? "text-failed-text" : "text-fg")}>{label}</span>
        <span className="type-mono-sm text-faint">{state === "done" ? "Done" : indeterminate ? "" : `${Math.round(pct)}%`}</span>
      </div>
      <div
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={indeterminate ? undefined : Math.round(pct)}
        aria-label={typeof label === "string" ? label : undefined}
        className="relative h-6 overflow-hidden rounded-pill bg-border-strong"
      >
        <div
          className={cx(
            "absolute inset-y-0 left-0 rounded-pill",
            failed ? "bg-failed" : state === "done" ? "bg-muted" : "bg-live",
            indeterminate && "w-1/3 animate-indeterminate motion-reduce:animate-none",
          )}
          style={indeterminate ? undefined : { width: `${pct}%` }}
        />
      </div>
      {detail && <p className={cx("type-mono-xs", failed ? "text-failed-text" : "text-faint")}>{detail}</p>}
    </div>
  );
}
