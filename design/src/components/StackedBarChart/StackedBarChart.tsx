import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Legend, type SeriesColour } from "../Legend/Legend";

export type ChartSeries = { key: string; label: string; colour: SeriesColour };

export type StackedBarChartProps = {
  title: ReactNode;
  /** Bottom of the stack first. */
  series: ChartSeries[];
  /** One bar per day: a label (shown on some days) and a value per series key. */
  days: { label?: string; values: Record<string, number> }[];
  /** Formats axis and tooltip values: "$0.60". */
  format?: (v: number) => string;
  /** A dated note: a dashed line over that day with its text. */
  annotation?: { index: number; label: ReactNode };
} & Omit<HTMLAttributes<HTMLElement>, "title">;

const FILL: Record<SeriesColour, string> = {
  ink: "bg-ink",
  "chart-1": "bg-chart-1",
  "chart-2": "bg-chart-2",
  "chart-3": "bg-chart-3",
  "chart-4": "bg-chart-4",
  "chart-5": "bg-chart-5",
};

/** Rounds up to 1, 2 or 5 times a power of ten, so the axis reads $1.00, not $0.83. */
function niceCeil(v: number) {
  const p = 10 ** Math.floor(Math.log10(v));
  return ([1, 2, 5, 10].find((m) => m * p >= v) ?? 10) * p;
}

/** Daily cost by activity (X70): stacked bars, a legend, and an optional dated marker. */
export function StackedBarChart({ title, series, days, format = (v) => `$${v.toFixed(2)}`, annotation, className, ...rest }: StackedBarChartProps) {
  const totals = days.map((d) => series.reduce((s, x) => s + (d.values[x.key] ?? 0), 0));
  const top = niceCeil(Math.max(0.01, ...totals));
  const ticks = [top, top / 2, 0];
  return (
    <section className={cx("flex flex-col gap-16 rounded-xl border border-border bg-surface px-20 py-16", className)} {...rest}>
      <header className="flex flex-wrap items-center justify-between gap-12">
        <h3 className="type-ui-strong text-fg">{title}</h3>
        <Legend items={series.map((s) => ({ label: s.label, colour: s.colour }))} />
      </header>
      <div className="grid grid-cols-[48px_1fr] gap-x-8">
        <div className="flex h-160 flex-col justify-between">
          {ticks.map((t, i) => (
            <span key={i} className="type-mono-xs text-faint">{format(t)}</span>
          ))}
        </div>
        <div className="relative h-160">
          {ticks.map((_, i) => (
            <span key={i} aria-hidden className="absolute inset-x-0 h-px bg-border" style={{ top: `${(i / (ticks.length - 1)) * 100}%` }} />
          ))}
          <ol className="absolute inset-0 flex items-end gap-4" aria-label={typeof title === "string" ? title : "Chart"}>
            {days.map((d, i) => (
              <li
                key={i}
                className="relative flex h-full min-w-0 flex-1 flex-col-reverse"
                aria-label={`${d.label ?? `Day ${i + 1}`}: ${format(totals[i])}, ${series.map((s) => `${s.label} ${format(d.values[s.key] ?? 0)}`).join(", ")}`}
              >
                {series.map((s) => {
                  const v = d.values[s.key] ?? 0;
                  return v > 0 ? (
                    <span key={s.key} className={cx("w-full first:rounded-b-xs last:rounded-t-xs", FILL[s.colour])} style={{ height: `${(v / top) * 100}%` }} />
                  ) : null;
                })}
                {annotation?.index === i && (
                  <span aria-hidden className="absolute -top-8 bottom-0 left-1/2 w-0 border-l border-dashed border-retrying" />
                )}
              </li>
            ))}
          </ol>
          {annotation && (
            <span
              className="absolute -top-14 pl-6 type-mono-xs whitespace-nowrap text-retrying"
              style={{ left: `${((annotation.index + 0.5) / days.length) * 100}%` }}
            >
              {annotation.label}
            </span>
          )}
        </div>
        <span />
        <div className="flex gap-4 pt-6">
          {days.map((d, i) => (
            <span key={i} className="min-w-0 flex-1 overflow-visible text-center type-mono-xs whitespace-nowrap text-faint">
              {d.label ?? ""}
            </span>
          ))}
        </div>
      </div>
    </section>
  );
}
