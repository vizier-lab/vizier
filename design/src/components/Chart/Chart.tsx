import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Legend, type SeriesColour } from "../Legend/Legend";

export type ChartKind = "bar" | "line" | "area" | "pie";

export type ChartDatum = {
  label: string;
  value: number;
  /** Which series the value belongs to; omit for a single series. */
  series?: string;
};

export type ChartProps = {
  kind: ChartKind;
  data: ChartDatum[];
  legend?: boolean;
  /** Plot height in px. */
  height?: number;
  /** Makes each bar, point column or slice a button; receives the datum pressed. */
  onPress?: (datum: ChartDatum) => void;
  /** Names the chart for screen readers; the data is also offered as a table. */
  title?: ReactNode;
} & Omit<HTMLAttributes<HTMLElement>, "title">;

const COLOURS: Exclude<SeriesColour, "ink">[] = ["chart-1", "chart-2", "chart-3", "chart-4", "chart-5"];
const BG: Record<string, string> = {
  "chart-1": "bg-chart-1",
  "chart-2": "bg-chart-2",
  "chart-3": "bg-chart-3",
  "chart-4": "bg-chart-4",
  "chart-5": "bg-chart-5",
};
const FILL: Record<string, string> = {
  "chart-1": "fill-chart-1",
  "chart-2": "fill-chart-2",
  "chart-3": "fill-chart-3",
  "chart-4": "fill-chart-4",
  "chart-5": "fill-chart-5",
};
const STROKE: Record<string, string> = {
  "chart-1": "stroke-chart-1",
  "chart-2": "stroke-chart-2",
  "chart-3": "stroke-chart-3",
  "chart-4": "stroke-chart-4",
  "chart-5": "stroke-chart-5",
};

/** What the plot is drawn from: categories in first-seen order, series likewise. */
export function shape(data: ChartDatum[]) {
  const categories = [...new Set(data.map((d) => d.label))];
  const series = [...new Set(data.map((d) => d.series ?? ""))];
  const at = (label: string, s: string) => data.find((d) => d.label === label && (d.series ?? "") === s);
  const max = Math.max(0, ...data.map((d) => (Number.isFinite(d.value) ? d.value : 0)));
  return { categories, series, at, max };
}

const colourOf = (i: number) => COLOURS[i % COLOURS.length];

function slicePath(from: number, to: number) {
  const a = (t: number) => [Math.sin(t * 2 * Math.PI), -Math.cos(t * 2 * Math.PI)];
  const [x0, y0] = a(from);
  const [x1, y1] = a(to);
  return `M0 0 L${x0} ${y0} A1 1 0 ${to - from > 0.5 ? 1 : 0} 1 ${x1} ${y1} Z`;
}

/** Bar, line, area and pie charts for a small set of values, with a text alternative. */
export function Chart({ kind, data, legend, height = 200, onPress, title, className, ...rest }: ChartProps) {
  const { categories, series, at, max } = shape(data);
  const named = series.filter(Boolean);
  const showLegend = legend ?? (kind === "pie" || named.length > 1);
  const legendItems =
    kind === "pie"
      ? categories.map((label, i) => ({ label, colour: colourOf(i) }))
      : series.map((s, i) => ({ label: s || "Value", colour: colourOf(i) }));
  const scale = (v: number) => (max > 0 ? Math.max(0, v) / max : 0);

  const plot = (() => {
    if (kind === "pie") {
      const total = data.reduce((sum, d) => sum + Math.max(0, d.value), 0);
      let acc = 0;
      return (
        <svg viewBox="-1 -1 2 2" className="mx-auto aspect-square" style={{ height }} aria-hidden>
          {data.map((d, i) => {
            const share = total > 0 ? Math.max(0, d.value) / total : 0;
            const from = acc;
            acc += share;
            if (share === 0) return null;
            const props = onPress ? { role: "button", tabIndex: 0, "aria-label": `${d.label}: ${d.value}`, onClick: () => onPress(d), onKeyDown: (e: React.KeyboardEvent) => (e.key === "Enter" || e.key === " ") && onPress(d) } : {};
            return share >= 1 ? (
              <circle key={i} r={1} className={cx(FILL[colourOf(i)], onPress && "cursor-pointer")} {...props} />
            ) : (
              <path key={i} d={slicePath(from, acc)} className={cx(FILL[colourOf(i)], "stroke-ground", onPress && "cursor-pointer")} strokeWidth={0.02} {...props} />
            );
          })}
        </svg>
      );
    }

    if (kind === "bar") {
      return (
        <div className="flex items-end gap-8" style={{ height }}>
          {categories.map((label) => (
            <div key={label} className="flex h-full min-w-0 flex-1 items-end justify-center gap-2">
              {series.map((s, si) => {
                const d = at(label, s);
                if (!d) return null;
                const bar = (
                  <span
                    className={cx("block w-full rounded-t-xs", BG[colourOf(si)], onPress && "cursor-pointer transition-opacity hover:opacity-80")}
                    style={{ height: `${scale(d.value) * 100}%` }}
                  />
                );
                return onPress ? (
                  <button key={s} type="button" aria-label={`${label}${s ? `, ${s}` : ""}: ${d.value}`} onClick={() => onPress(d)} className="flex h-full min-w-0 flex-1 items-end">
                    {bar}
                  </button>
                ) : (
                  <span key={s} className="flex h-full min-w-0 flex-1 items-end">
                    {bar}
                  </span>
                );
              })}
            </div>
          ))}
        </div>
      );
    }

    // line and area: one polyline per series over evenly spaced categories.
    const x = (i: number) => (categories.length < 2 ? 50 : (i / (categories.length - 1)) * 100);
    const y = (v: number) => 100 - scale(v) * 100;
    return (
      <div className="relative" style={{ height }}>
        <svg viewBox="0 0 100 100" preserveAspectRatio="none" className="size-full overflow-visible" aria-hidden>
          <line x1={0} y1={100} x2={100} y2={100} className="stroke-border" strokeWidth={1} style={{ vectorEffect: "non-scaling-stroke" }} />
          {series.map((s, si) => {
            const pts = categories.flatMap((label, i) => {
              const d = at(label, s);
              return d ? [[x(i), y(d.value)] as const] : [];
            });
            if (pts.length === 0) return null;
            const line = pts.map(([px, py]) => `${px},${py}`).join(" ");
            return (
              <g key={s}>
                {kind === "area" && <polygon points={`${pts[0][0]},100 ${line} ${pts[pts.length - 1][0]},100`} className={cx(FILL[colourOf(si)], "opacity-20")} />}
                <polyline points={line} fill="none" strokeWidth={2} strokeLinejoin="round" className={STROKE[colourOf(si)]} style={{ vectorEffect: "non-scaling-stroke" }} />
              </g>
            );
          })}
        </svg>
        {onPress && (
          <div className="absolute inset-0 flex">
            {categories.map((label) => (
              <button
                key={label}
                type="button"
                aria-label={`${label}: ${at(label, series[0])?.value ?? ""}`}
                onClick={() => {
                  const d = at(label, series[0]);
                  if (d) onPress(d);
                }}
                className="h-full flex-1 cursor-pointer hover:bg-raised/40"
              />
            ))}
          </div>
        )}
      </div>
    );
  })();

  return (
    <figure className={cx("flex min-w-0 flex-col gap-12", className)} {...rest}>
      {title && <figcaption className="type-ui-strong text-fg">{title}</figcaption>}
      {plot}
      {kind !== "pie" && (
        <div aria-hidden className="flex gap-8">
          {categories.map((label) => (
            <span key={label} className="min-w-0 flex-1 truncate text-center type-mono-xs text-faint">
              {label}
            </span>
          ))}
        </div>
      )}
      {showLegend && <Legend items={legendItems} className="justify-center" />}
      <table className="sr-only">
        <caption>{typeof title === "string" ? title : "Chart data"}</caption>
        <thead>
          <tr>
            <th>Label</th>
            {named.length > 0 && <th>Series</th>}
            <th>Value</th>
          </tr>
        </thead>
        <tbody>
          {data.map((d, i) => (
            <tr key={i}>
              <td>{d.label}</td>
              {named.length > 0 && <td>{d.series ?? ""}</td>}
              <td>{d.value}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </figure>
  );
}
