import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import type { SeriesColour } from "../Legend/Legend";

const FILL: Record<SeriesColour, string> = {
  ink: "bg-ink",
  "chart-1": "bg-chart-1",
  "chart-2": "bg-chart-2",
  "chart-3": "bg-chart-3",
  "chart-4": "bg-chart-4",
  "chart-5": "bg-chart-5",
};

export type MeterProps = {
  /** 0–1. */
  share: number;
  colour?: SeriesColour;
  /** Shown after the bar: "61%". */
  label?: ReactNode;
} & HTMLAttributes<HTMLSpanElement>;

/** One share bar, for a share column in a table. */
export function Meter({ share, colour = "ink", label, className, ...rest }: MeterProps) {
  const pct = Math.round(Math.max(0, Math.min(1, share)) * 100);
  return (
    <span className={cx("flex items-center gap-8", className)} {...rest}>
      <span role="meter" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct} aria-label="Share" className="relative h-4 min-w-60 flex-1 overflow-hidden rounded-pill bg-border-strong">
        <span className={cx("absolute inset-y-0 left-0 rounded-pill", FILL[colour])} style={{ width: `${pct}%` }} />
      </span>
      <span className="w-32 text-right type-mono-xs text-faint">{label ?? `${pct}%`}</span>
    </span>
  );
}

export type BarListProps = {
  items: { label: ReactNode; share: number; value: ReactNode; colour?: SeriesColour }[];
  /** The rest, folded: "Other · 14". */
  other?: { label: ReactNode; share: number; value: ReactNode };
} & HTMLAttributes<HTMLUListElement>;

/** Label, bar, value for each item, with the rest folded into "Other" (X71). */
export function BarList({ items, other, className, ...rest }: BarListProps) {
  const rows = other ? [...items, { ...other, colour: "chart-5" as SeriesColour }] : items;
  return (
    <ul className={cx("flex flex-col gap-10", className)} {...rest}>
      {rows.map((r, i) => (
        <li key={i} className="grid grid-cols-[minmax(0,1fr)_minmax(0,2fr)_max-content] items-center gap-12">
          <span className={cx("truncate type-ui", other && i === rows.length - 1 ? "text-muted" : "text-fg")}>{r.label}</span>
          <Meter share={r.share} colour={r.colour} />
          <span className="type-mono-sm text-fg">{r.value}</span>
        </li>
      ))}
    </ul>
  );
}
