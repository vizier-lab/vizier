import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type CallStatus = "pending" | "ok" | "error" | "deferred" | "timeout" | "disconnected" | "cancelled" | "interrupted";

export type CallChipProps = {
  status: CallStatus;
  /** "pick_files · 2 files chosen". */
  detail?: ReactNode;
} & HTMLAttributes<HTMLDivElement>;

const STATUS: Record<CallStatus, { dot: string; word: string; text: string }> = {
  pending: { dot: "bg-live", word: "waiting", text: "text-fg" },
  ok: { dot: "bg-faint", word: "done", text: "text-muted" },
  error: { dot: "bg-failed", word: "failed", text: "text-failed-text" },
  deferred: { dot: "bg-retrying", word: "deferred", text: "text-retrying" },
  timeout: { dot: "bg-retrying", word: "timed out", text: "text-retrying" },
  disconnected: { dot: "bg-failed", word: "disconnected", text: "text-failed-text" },
  cancelled: { dot: "bg-unlit", word: "cancelled", text: "text-muted" },
  interrupted: { dot: "bg-unlit", word: "interrupted", text: "text-muted" },
};

/** The outcome of a client tool call, inside the reply (D58). */
export function CallChip({ status, detail, className, ...rest }: CallChipProps) {
  const s = STATUS[status];
  return (
    <div className={cx("flex items-center gap-8 rounded-md bg-surface px-10 py-6", className)} {...rest}>
      <span aria-hidden className={cx("size-6 shrink-0 rounded-pill", s.dot)} />
      <span className={cx("type-mono-sm-medium", s.text)}>{s.word}</span>
      {detail && <span className="min-w-0 truncate type-small text-faint">{detail}</span>}
    </div>
  );
}

export const callStatuses = Object.keys(STATUS) as CallStatus[];
