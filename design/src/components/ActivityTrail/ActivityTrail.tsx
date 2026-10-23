import { useState, type HTMLAttributes, type ReactNode } from "react";
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
  /** Mobile: one line with a dot per call instead of the spine. */
  mini?: { dots: ("done" | "running" | "failed")[] };
} & Omit<HTMLAttributes<HTMLDivElement>, "title">;

/** A turn's work, on a timeline (D49): TrailHead over a spine of calls. */
export function ActivityTrail({ state, title, counts, defaultExpanded, children, mini, className, ...rest }: ActivityTrailProps) {
  const [expanded, setExpanded] = useState(defaultExpanded ?? state === "live");
  if (mini) {
    return (
      <div className={cx("flex items-center gap-8 rounded-lg border border-border bg-surface px-12 py-8", className)} {...rest}>
        <span className="type-small-medium text-fg">{title}</span>
        <span aria-hidden className="flex items-center gap-4">
          {mini.dots.map((d, i) => (
            <span key={i} className={cx("size-5 rounded-pill", d === "running" ? "bg-live" : d === "failed" ? "bg-failed" : "bg-muted")} />
          ))}
        </span>
        {counts && <span className="min-w-0 truncate type-mono-xs text-faint">{counts}</span>}
      </div>
    );
  }
  return (
    <div className={cx("flex flex-col gap-10 rounded-lg border border-border bg-surface px-16 py-12", className)} {...rest}>
      <TrailHead state={state} title={title} counts={counts} expanded={expanded} onClick={() => setExpanded(!expanded)} />
      {expanded && children && (
        <ul className="relative flex flex-col before:absolute before:top-14 before:bottom-14 before:left-6 before:w-px before:bg-border-strong before:content-['']">
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
