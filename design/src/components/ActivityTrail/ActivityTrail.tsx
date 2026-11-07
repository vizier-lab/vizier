import { Children, useState, type HTMLAttributes, type ReactNode } from "react";
import { cx } from "../../lib/cx";
import { TrailHead, type TrailHeadProps } from "../TrailHead/TrailHead";

export type ActivityTrailProps = {
  state: TrailHeadProps["state"];
  title: ReactNode;
  counts?: ReactNode;
  /** Open by default while live; a finished turn folds to its head. */
  defaultExpanded?: boolean;
  /** TrailCalls, Thoughts, a Fold; Notices for failures. */
  children?: ReactNode;
  /**
   * Mobile: one line instead of the spine. A dot per call, at most a few of each state, and `more`
   * for the calls past that — a dot for every call would stretch the line with the work done.
   */
  mini?: { dots: ("done" | "running" | "failed")[]; more?: number };
  /** Inside an agent's turn, as the start of its reply: no card around it. */
  bare?: boolean;
} & Omit<HTMLAttributes<HTMLDivElement>, "title">;

/** A turn's work, on a timeline (D49): TrailHead over a spine of calls. */
export function ActivityTrail({ state, title, counts, defaultExpanded, children, mini, bare, className, ...rest }: ActivityTrailProps) {
  const empty = Children.toArray(children).length === 0;
  const [expanded, setExpanded] = useState(defaultExpanded ?? state === "live");
  if (mini) {
    return (
      <div className={cx("flex min-w-0 flex-col gap-4 rounded-lg border border-border bg-surface px-12 py-8", className)} {...rest}>
        <span className="flex min-w-0 items-center gap-8">
          <span className="shrink-0 type-small-medium text-fg">{title}</span>
          {/* Clipped by the row, so the strip can never push the count off the end. */}
          <span className="flex min-w-0 flex-1 justify-end overflow-hidden">
            <span aria-hidden className="flex shrink-0 items-center gap-4">
              {mini.dots.map((d, i) => (
                <span key={i} className={cx("size-5 rounded-pill", d === "running" ? "bg-live" : d === "failed" ? "bg-failed" : "bg-muted")} />
              ))}
            </span>
            {mini.more ? <span className="ml-6 shrink-0 type-mono-xs text-faint">+{mini.more}</span> : null}
          </span>
        </span>
        {/* Its own line: beside the title and a full strip there is no room left for it. */}
        {counts && <span className="min-w-0 truncate type-mono-xs text-faint">{counts}</span>}
      </div>
    );
  }
  return (
    <div className={cx("flex flex-col gap-10", !bare && "rounded-lg border border-border bg-surface px-16 py-12", className)} {...rest}>
      <TrailHead state={state} title={title} counts={counts} expanded={expanded} plain={empty} onClick={() => setExpanded(!expanded)} />
      {expanded && !empty && (
        // While live, each step rises in as it arrives. It is the li itself that animates, so the spine's
        // not-last rule still sees its siblings.
        <ul className={cx("relative flex flex-col", state === "live" && "[&>li]:animate-rise-in motion-reduce:[&>li]:animate-none")}>
          {children}
        </ul>
      )}
    </div>
  );
}

/** A non-call item on the spine (a Thought, a Notice), aligned with the call names. */
export function TrailItem({ className, ...rest }: HTMLAttributes<HTMLLIElement>) {
  return <li className={cx("relative py-4 pl-24", className)} {...rest} />;
}
