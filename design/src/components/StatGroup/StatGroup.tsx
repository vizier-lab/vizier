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

/**
 * A strip of stat tiles (X11): label, value, sub-line. Where the strip can't hold every tile at
 * 160px it wraps into rows; each tile draws its top and left rule, and the strip clips the ones
 * on its outer edge, so only the rules between tiles show however it wraps.
 */
export function StatGroup({ stats, className, ...rest }: StatGroupProps) {
  return (
    <dl className={cx("grid grid-cols-[repeat(auto-fit,minmax(160px,1fr))] overflow-hidden rounded-xl border border-border bg-surface", className)} {...rest}>
      {stats.map((s, i) => (
        <div key={i} className="-mt-px -ml-px flex flex-col gap-4 border-t border-l border-border px-20 py-16">
          <dt className="type-small text-faint">{s.label}</dt>
          <dd className={cx("m-0 type-title", s.tone === "failed" ? "text-failed" : "text-fg")}>{s.value}</dd>
          {s.sub && <dd className="m-0 type-mono-xs text-faint">{s.sub}</dd>}
        </div>
      ))}
    </dl>
  );
}
