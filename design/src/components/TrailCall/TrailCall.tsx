import { useState, type HTMLAttributes, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type TrailCallState = "done" | "running" | "failed" | "limit" | "lookup";

export type TrailCallProps = {
  /** The tool: "execute_python", "list_tool_functions". Optional when `label` says it in words. */
  name?: string;
  /** What the model said it was for. */
  intent?: ReactNode;
  /**
   * The row in plain words, in place of `name` and `intent`: a run's intent ("Check what we
   * discussed last week"), or a sentence built from what it used.
   */
  label?: ReactNode;
  /** `soft` for a label the trail made up (no intent was stated), `default` otherwise. */
  labelTone?: "default" | "soft";
  /** What the call used, beside its time (CallIcons). */
  icons?: ReactNode;
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
export function TrailCall({
  name,
  intent,
  label,
  labelTone = "default",
  icons,
  duration,
  state = "done",
  children,
  defaultOpen = false,
  className,
  ...rest
}: TrailCallProps) {
  const lookup = state === "lookup";
  const [open, setOpen] = useState(defaultOpen);
  const dot = (
    <span aria-hidden className="relative z-10 flex size-12 shrink-0 items-center justify-center">
      <span className={cx("size-7 rounded-pill ring-3 ring-surface", DOT[state])} />
    </span>
  );
  const words =
    label != null ? (
      <span className={cx("min-w-0 flex-1 truncate type-small", labelTone === "soft" ? "text-soft" : "text-fg")}>{label}</span>
    ) : (
      <>
        {name && <span className={cx(lookup ? "type-mono-sm text-muted" : "type-mono-sm-medium text-fg")}>{name}</span>}
        {intent && <span className="min-w-0 flex-1 truncate type-small text-muted">{intent}</span>}
      </>
    );
  const time = !lookup && (
    <span className={cx("shrink-0 type-mono-xs", icons ? "min-w-48 text-right" : "ml-auto", DURATION[state])}>
      {state === "running" ? "running…" : duration}
      {state === "failed" && <span className="sr-only"> (failed)</span>}
    </span>
  );
  const chevron = children && (
    <Icon name={open ? "chevron-down" : "chevron-right"} size={13} tone="faint" className={cx("shrink-0", lookup && !icons && "ml-auto")} />
  );

  let row: ReactNode;
  if (icons) {
    // The icons open a popup of their own, so they cannot sit inside the disclosure button: the
    // button holds the dot and the words and stretches over the whole row, and the icons sit
    // above it.
    row = (
      <div className="relative flex min-h-28 items-center gap-10">
        {children ? (
          <button
            type="button"
            aria-expanded={open}
            onClick={() => setOpen(!open)}
            className="flex min-w-0 flex-1 items-center gap-10 rounded-xs text-left after:absolute after:inset-0 after:content-['']"
          >
            {dot}
            {words}
          </button>
        ) : (
          <span className="flex min-w-0 flex-1 items-center gap-10">
            {dot}
            {words}
          </span>
        )}
        <span className="relative z-10 ml-auto flex shrink-0 items-center">{icons}</span>
        {time}
        {/* A row with nothing to open keeps the chevron's place, so the times stay in one column. */}
        {chevron || <span aria-hidden className="size-13 shrink-0" />}
      </div>
    );
  } else {
    const inner = (
      <>
        {dot}
        {words}
        {time}
        {chevron}
      </>
    );
    row = children ? (
      <button type="button" aria-expanded={open} onClick={() => setOpen(!open)} className="flex min-h-28 w-full items-center gap-10 rounded-xs text-left">
        {inner}
      </button>
    ) : (
      <div className="flex min-h-28 items-center gap-10">{inner}</div>
    );
  }

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
      {row}
      {children && open && <div className="flex flex-col gap-4 pb-6 pl-24">{children}</div>}
    </li>
  );
}
