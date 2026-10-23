import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type Stat = {
  label: ReactNode;
  value: ReactNode;
  /** Mono sub-line: "+12% vs previous 30 days". */
  sub?: ReactNode;
  /** Failed: the value in ember (failed turns). */
  tone?: "default" | "failed";
};

export type StatGroupProps = { stats: Stat[] } & HTMLAttributes<HTMLDListElement>;

/** A strip of stat tiles (X11): label, value, sub-line. */
export function StatGroup({ stats, className, ...rest }: StatGroupProps) {
  return (
    <dl className={cx("grid auto-cols-fr grid-flow-col divide-x divide-border rounded-xl border border-border bg-surface", className)} {...rest}>
      {stats.map((s, i) => (
        <div key={i} className="flex flex-col gap-4 px-20 py-16">
          <dt className="type-small text-faint">{s.label}</dt>
          <dd className={cx("m-0 type-title", s.tone === "failed" ? "text-failed" : "text-fg")}>{s.value}</dd>
          {s.sub && <dd className="m-0 type-mono-xs text-faint">{s.sub}</dd>}
        </div>
      ))}
    </dl>
  );
}
