import { useState, type HTMLAttributes, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
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
  /** Calls made inside it (ScriptCalls), a notice, or a traceback, under the row. They fold away: the row opens them. */
  children?: ReactNode;
  /** Start with the details open (a failure you want seen). Default: folded. */
  defaultOpen?: boolean;
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
  running: "text-live-text",
  failed: "text-failed",
  limit: "text-retrying",
  lookup: "text-faint",
};

/**
 * One model call on the trail spine (D51). Lookups read quieter: no result, no time. A call with
 * details is a disclosure, folded by default; the spine runs from its dot down to the next call
 * and stops there, so the last call has no line under it.
 */
export function TrailCall({ name, intent, duration, state = "done", children, defaultOpen = false, className, ...rest }: TrailCallProps) {
  const lookup = state === "lookup";
  const [open, setOpen] = useState(defaultOpen);
  const row = (
    <>
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
      {children && <Icon name={open ? "chevron-down" : "chevron-right"} size={13} tone="faint" className={cx("shrink-0", lookup && "ml-auto")} />}
    </>
  );
  return (
    <li
      className={cx(
        "relative flex flex-col",
        // The spine: from this dot to the next call's dot. Nothing under the last one.
        "not-last:after:absolute not-last:after:top-20 not-last:after:-bottom-8 not-last:after:left-6 not-last:after:w-px not-last:after:bg-border-strong not-last:after:content-['']",
        className,
      )}
      {...rest}
    >
      {children ? (
        <button
          type="button"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
          className={cx("flex min-h-28 w-full items-center gap-10 rounded-xs text-left", !intent && !lookup && "")}
        >
          {row}
        </button>
      ) : (
        <div className="flex min-h-28 items-center gap-10">{row}</div>
      )}
      {children && open && <div className="flex flex-col gap-4 pb-6 pl-24">{children}</div>}
    </li>
  );
}
