import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type Night = {
  /** Shown under some nights: "Sep 21", "today". */
  label?: string;
  /** Memories written or revised; 0 for a night with no dream. */
  memories: number;
  /** CORE changed that night: a ring around the dot. */
  core?: boolean;
};

export type NightStripProps = {
  nights: Night[];
  meta?: ReactNode;
} & HTMLAttributes<HTMLElement>;

/** The last 30 nights of dreams (X69): dot size is memories written or revised; a ring is CORE changed. */
export function NightStrip({ nights, meta = "dot size = memories written or revised · ring = CORE changed", className, ...rest }: NightStripProps) {
  const max = Math.max(1, ...nights.map((n) => n.memories));
  const size = (m: number) => (m <= 0 ? 3 : Math.max(5, Math.round((m / max) * 12)));
  return (
    <section className={cx("flex flex-col gap-16 rounded-xl border border-border bg-surface px-20 py-16", className)} {...rest}>
      <header className="flex items-baseline justify-between gap-12">
        <h3 className="type-ui-strong text-fg">Last {nights.length} nights</h3>
        {meta && <span className="type-mono-xs text-faint">{meta}</span>}
      </header>
      <div
        role="img"
        aria-label={`Dreams over the last ${nights.length} nights: ${nights.filter((n) => n.memories > 0).length} with changes, ${nights.filter((n) => n.core).length} changed CORE`}
        className="grid"
        style={{ gridTemplateColumns: `repeat(${nights.length}, minmax(0, 1fr))` }}
      >
        {nights.map((n, i) => (
          <span key={i} className="flex h-24 items-center justify-center">
            <span
              className={cx("rounded-pill", n.memories <= 0 ? "bg-unlit" : "bg-ink", n.core && "ring-2 ring-ink ring-offset-2 ring-offset-surface")}
              style={{ width: size(n.memories), height: size(n.memories) }}
            />
          </span>
        ))}
        {nights.map((n, i) => (
          <span key={`l${i}`} className="pt-6 text-center type-mono-xs whitespace-nowrap text-faint">
            {n.label ?? ""}
          </span>
        ))}
      </div>
    </section>
  );
}
