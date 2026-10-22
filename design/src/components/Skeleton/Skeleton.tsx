import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

export type SkeletonProps = {
  /** Card: a surface with a heading bar and lines. Lines: bare text lines (history, lists). */
  variant?: "card" | "lines";
  /** Number of text lines. */
  lines?: number;
} & HTMLAttributes<HTMLDivElement>;

// Line lengths cycle so the block reads as text, not a bar chart.
const WIDTHS = ["w-full", "w-5/6", "w-11/12", "w-2/3"];

/** A loading placeholder (D31). Announced once as "Loading". */
export function Skeleton({ variant = "card", lines = 3, className, ...rest }: SkeletonProps) {
  const bars = (
    <>
      {Array.from({ length: lines }, (_, i) => (
        <div key={i} className={cx("h-10 rounded-xs bg-raised", WIDTHS[i % WIDTHS.length])} />
      ))}
    </>
  );
  return (
    <div
      role="status"
      aria-label="Loading"
      className={cx(
        "flex flex-col gap-12 animate-shimmer motion-reduce:animate-none",
        variant === "card" && "rounded-xl border border-border bg-surface p-20",
        className,
      )}
      {...rest}
    >
      {variant === "card" && <div className="h-14 w-1/2 rounded-xs bg-raised" />}
      {bars}
    </div>
  );
}
