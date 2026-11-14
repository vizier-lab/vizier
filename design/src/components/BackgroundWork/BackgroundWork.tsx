import { useId, type HTMLAttributes, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type BackgroundWorkState = "running" | "succeeded" | "failed" | "timed_out" | "cancelled" | "interrupted";

export type BackgroundWorkProps = {
  /** The script's intent: what the agent said it is for. */
  intent: string;
  /** The handle, "d_k3x9a1b2". */
  handle: string;
  state: BackgroundWorkState;
  /** "0:41". */
  elapsed?: ReactNode;
  /** The line under the title. Left out, it is said from the state and `callCount`. */
  summary?: ReactNode;
  /** Calls made so far, for the default summary while running. */
  callCount?: number;
  /** Whether it was delivered to the agent yet (a final script only). */
  delivered?: boolean;
  expanded?: boolean;
  onExpandedChange?: (expanded: boolean) => void;
  /** The expanded body: the activity trail's `RunDetail` for this script. */
  children?: ReactNode;
} & Omit<HTMLAttributes<HTMLDivElement>, "children">;

const ENDED: Record<Exclude<BackgroundWorkState, "running">, string> = {
  succeeded: "Finished",
  failed: "Failed",
  timed_out: "Ran out of time",
  cancelled: "Cancelled by the agent · nothing delivered",
  interrupted: "Interrupted by a restart",
};

function defaultSummary(state: BackgroundWorkState, callCount?: number, delivered?: boolean): string {
  if (state === "running") {
    const n = callCount ?? 0;
    return n === 0 ? "Starting" : `${n} tool call${n === 1 ? "" : "s"} so far`;
  }
  if (state === "cancelled") return ENDED.cancelled;
  return `${ENDED[state]} · ${delivered ? "delivered to the agent" : "waiting to be delivered"}`;
}

/**
 * One detached script in the Background work panel (spec 058): titled by its intent, collapsed
 * to a line, and expanded to the activity trail's run detail. There is no cancel control: only
 * the agent cancels.
 */
export function BackgroundWork({
  intent,
  handle,
  state,
  elapsed,
  summary,
  callCount,
  delivered,
  expanded = false,
  onExpandedChange,
  children,
  className,
  ...rest
}: BackgroundWorkProps) {
  const bodyId = useId();
  const running = state === "running";
  const failed = state === "failed" || state === "timed_out" || state === "interrupted";
  return (
    <div
      className={cx(
        "flex flex-col rounded-md border bg-surface",
        expanded ? "border-live" : "border-border",
        className,
      )}
      {...rest}
    >
      <button
        type="button"
        aria-expanded={expanded}
        aria-controls={children ? bodyId : undefined}
        onClick={() => onExpandedChange?.(!expanded)}
        className="flex flex-col gap-4 rounded-md px-12 py-10 text-left hover:bg-raised focus-visible:outline-2 focus-visible:outline-focus"
      >
        <span className="flex items-center gap-8">
          <span
            aria-hidden
            className={cx("size-7 shrink-0 rounded-pill", running ? "bg-live" : failed ? "bg-failed" : "bg-faint")}
          />
          <span className="min-w-0 flex-1 truncate type-ui text-fg">{intent}</span>
          <span className="shrink-0 type-mono-sm text-faint">
            {handle}
            {elapsed != null && <> · {elapsed}</>}
          </span>
          <Icon name={expanded ? "chevron-down" : "chevron-right"} size={14} className="shrink-0 text-soft" />
        </span>
        <span className={cx("type-small", failed ? "text-failed" : "text-faint")}>
          {summary ?? defaultSummary(state, callCount, delivered)}
        </span>
      </button>
      {expanded && children && (
        <div id={bodyId} className="px-12 pb-12">
          {children}
        </div>
      )}
    </div>
  );
}
