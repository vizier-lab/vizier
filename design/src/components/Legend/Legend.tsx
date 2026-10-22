import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

/** The colours a chart series may take: the five data colours, or ink for the main series. */
export type SeriesColour = "chart-1" | "chart-2" | "chart-3" | "chart-4" | "chart-5" | "ink";

const SWATCH: Record<SeriesColour, string> = {
  "chart-1": "bg-chart-1",
  "chart-2": "bg-chart-2",
  "chart-3": "bg-chart-3",
  "chart-4": "bg-chart-4",
  "chart-5": "bg-chart-5",
  ink: "bg-ink",
};

export type LegendProps = {
  items: { label: ReactNode; colour: SeriesColour; value?: ReactNode }[];
} & HTMLAttributes<HTMLUListElement>;

/** Swatch + label for each chart series (X72). */
export function Legend({ items, className, ...rest }: LegendProps) {
  return (
    <ul className={cx("flex flex-wrap items-center gap-x-16 gap-y-6", className)} {...rest}>
      {items.map((item, i) => (
        <li key={i} className="flex items-center gap-6 type-small text-muted">
          <span aria-hidden className={cx("size-9 shrink-0 rounded-xs", SWATCH[item.colour])} />
          {item.label}
          {item.value != null && <span className="type-mono-xs text-faint">{item.value}</span>}
        </li>
      ))}
    </ul>
  );
}
