import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type TrailCallState = "done" | "running" | "failed" | "limit" | "lookup";

export type TrailCallProps = {
  /** The tool: "execute_python", "list_tool_functions". */
  name: string;
  /** What the model said it was for. */
  intent?: ReactNode;
  /** "12.6s"; replaced by "running…" while it runs; lookups show none. */
  duration?: ReactNode;
  state?: TrailCallState;
  /** Calls made inside it (ScriptCalls), a notice, or a traceback, under the row. */
  children?: ReactNode;
} & HTMLAttributes<HTMLLIElement>;

const DOT: Record<TrailCallState, string> = {
  done: "bg-muted",
  running: "bg-live",
  failed: "bg-failed",
  limit: "bg-retrying",
  lookup: "bg-faint",
};

const DURATION: Record<TrailCallState, string> = {
  done: "text-faint",
  running: "text-live",
  failed: "text-failed",
  limit: "text-retrying",
  lookup: "text-faint",
};

/** One model call on the trail spine (D51). Lookups read quieter: no result, no time. */
export function TrailCall({ name, intent, duration, state = "done", children, className, ...rest }: TrailCallProps) {
  const lookup = state === "lookup";
  return (
    <li className={cx("relative flex flex-col", className)} {...rest}>
      <div className="flex min-h-28 items-center gap-10">
        <span aria-hidden className="relative z-10 flex size-12 shrink-0 items-center justify-center">
          <span className={cx("size-7 rounded-pill ring-3 ring-surface", DOT[state])} />
        </span>
        <span className={cx(lookup ? "type-mono-sm text-muted" : "type-mono-sm-medium text-fg")}>{name}</span>
        {intent && <span className="min-w-0 flex-1 truncate type-small text-muted">{intent}</span>}
        {!lookup && (
          <span className={cx("ml-auto shrink-0 type-mono-xs", DURATION[state])}>
            {state === "running" ? "running…" : duration}
            {state === "failed" && <span className="sr-only"> (failed)</span>}
          </span>
        )}
      </div>
      {children && <div className="flex flex-col gap-4 pb-6 pl-24">{children}</div>}
    </li>
  );
}
