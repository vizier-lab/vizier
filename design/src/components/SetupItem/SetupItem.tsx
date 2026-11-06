import type { HTMLAttributes, ReactNode } from "react";
import { Sigil } from "../../brand/Sigil/Sigil";
import { cx } from "../../lib/cx";

export type SetupItemProps = {
  agentId: string;
  agentName: string;
  /** What the agent is doing now: "Downloading a model". */
  step: ReactNode;
  /** 0 to 100. Omit when the size is unknown: the bar then moves without a position. */
  percent?: number;
  /** Under the step: "412 MB of 1.2 GB", or what went wrong. */
  detail?: ReactNode;
  /** Time left, drawn after the detail: "about 2 min". */
  eta?: ReactNode;
  state?: "running" | "failed";
} & Omit<HTMLAttributes<HTMLDivElement>, "children">;

/**
 * An agent's live setup in the notifications popup, drawn in the same card shape as a
 * notification so the two read as one list. It has no read state and is never counted as unread.
 */
export function SetupItem({ agentId, agentName, step, percent, detail, eta, state = "running", className, ...rest }: SetupItemProps) {
  const failed = state === "failed";
  const indeterminate = percent === undefined && !failed;
  const pct = Math.max(0, Math.min(100, percent ?? 0));
  return (
    <div className={cx("flex w-full min-w-0 items-start gap-12 rounded-lg border border-border bg-raised px-12 py-12", className)} {...rest}>
      <Sigil agentId={agentId} size="md" label={agentName} />
      <div className="flex min-w-0 flex-1 flex-col gap-3">
        <div className="flex items-baseline justify-between gap-8">
          <span className={cx("min-w-0 truncate type-ui-strong", failed ? "text-failed-text" : "text-fg")}>
            {agentName} · {step}
          </span>
          {!indeterminate && !failed && <span className="type-small-medium text-faint">{Math.round(pct)}%</span>}
        </div>
        {(detail || eta) && (
          <span className={cx("type-small", failed ? "text-failed-text" : "text-muted")}>
            {detail}
            {detail && eta ? " · " : null}
            {eta}
          </span>
        )}
        <div
          role="progressbar"
          aria-label={`${agentName} setup`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={indeterminate ? undefined : Math.round(pct)}
          className="relative mt-3 h-3 overflow-hidden rounded-pill bg-border-strong"
        >
          <div
            className={cx(
              "absolute inset-y-0 left-0 rounded-pill",
              failed ? "bg-failed" : "bg-live",
              indeterminate && "w-1/3 animate-indeterminate motion-reduce:animate-none",
            )}
            style={indeterminate ? undefined : { width: `${failed ? Math.max(pct, 4) : pct}%` }}
          />
        </div>
      </div>
    </div>
  );
}
