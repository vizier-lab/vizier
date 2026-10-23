import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { TextLink } from "../TextLink/TextLink";

export type BackgroundJobState = "running" | "reporting" | "reported" | "undelivered" | "cancelled" | "interrupted";

export type BackgroundJobProps = {
  /** "delegate_agent → Scout", "paralel_subtasks". */
  title: string;
  /** What it was asked to do. */
  goal: ReactNode;
  state: BackgroundJobState;
  /** "1:42". */
  elapsed?: ReactNode;
  /** Pieces done and in all: drawn as dots, "2 of 3 pieces done". */
  pieces?: { done: number; total: number };
  onJumpToStart?: () => void;
  onCancel?: () => void;
} & HTMLAttributes<HTMLDivElement>;

const LINE: Partial<Record<BackgroundJobState, ReactNode>> = {
  reporting: "reporting back",
  reported: "Reported",
  undelivered: "Finished, but the report couldn't be delivered",
  cancelled: "Cancelled by you · no report will be sent",
  interrupted: "The server restarted while this ran.",
};

/** One job in the background tray (D59). Cancelled jobs never report; restarts mark them lost. */
export function BackgroundJob({ title, goal, state, elapsed, pieces, onJumpToStart, onCancel, className, ...rest }: BackgroundJobProps) {
  const active = state === "running" || state === "reporting";
  return (
    <div className={cx("flex flex-col gap-6 rounded-md border border-border bg-surface px-12 py-10", className)} {...rest}>
      <div className="flex items-center gap-8">
        {state !== "cancelled" && state !== "interrupted" && (
          <span aria-hidden className={cx("size-7 shrink-0 rounded-pill", active ? "bg-live" : state === "undelivered" ? "bg-retrying" : "bg-faint")} />
        )}
        <span className="min-w-0 flex-1 truncate type-mono-sm-medium text-fg">{title}</span>
        {state === "cancelled" ? (
          <span className="inline-flex items-center gap-4 type-mono-sm text-faint">
            <Icon name="ban" size={13} /> cancelled
          </span>
        ) : state === "interrupted" ? (
          <span className="inline-flex items-center gap-4 type-mono-sm text-retrying">
            <Icon name="alert" size={12} /> lost
          </span>
        ) : (
          elapsed && <span className="type-mono-sm text-faint">{elapsed}</span>
        )}
      </div>
      <p className="type-small-para text-soft">{goal}</p>
      {pieces && active && (
        <div className="flex items-center gap-6">
          <span aria-hidden className="flex gap-4">
            {Array.from({ length: pieces.total }, (_, i) => (
              <span key={i} className={cx("size-6 rounded-pill", i < pieces.done ? "bg-ink" : "border border-dashed border-faint")} />
            ))}
          </span>
          <span className="type-small text-faint">
            {pieces.done} of {pieces.total} pieces done{state === "reporting" && " · reporting back"}
          </span>
        </div>
      )}
      {!active && LINE[state] && <p className={cx("type-small", state === "undelivered" ? "text-retrying" : "text-faint")}>{LINE[state]}</p>}
      {state === "running" && (onJumpToStart || onCancel) && (
        <div className="flex items-center gap-14">
          {onJumpToStart && <TextLink onClick={onJumpToStart}>Jump to start</TextLink>}
          {onCancel && <TextLink tone="danger" onClick={onCancel}>Cancel</TextLink>}
        </div>
      )}
    </div>
  );
}
