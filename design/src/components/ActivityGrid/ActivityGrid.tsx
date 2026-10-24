import type { HTMLAttributes, ReactNode } from "react";
import { motion } from "motion/react";
import { cx } from "../../lib/cx";
import { motionDurations, motionEases } from "../../tokens/tokens";

export type ActivityRow = {
  label: string;
  /** Turns per hour, oldest first; 24 values. */
  hours: number[];
  /** Replying right now: the newest dot turns live. */
  live?: boolean;
  /** The "All N" total row: set apart. */
  total?: boolean;
  /** One tooltip per hour ("14:00 · 3 turns"); 24 values. Gives every dot a name and a place in the tab order. */
  hints?: string[];
};

export type ActivityGridProps = {
  rows: ActivityRow[];
  /** Right of the title: "4 agents · dot size = turns per hour". */
  meta?: ReactNode;
  /** One agent: under the grid, what the day was in words. */
  summary?: ReactNode;
  /** No agents: the message and its one action. */
  empty?: { title: ReactNode; message: ReactNode; action?: ReactNode };
  /** Under the grid: the Most active · Mine · Live · Failed switch, "995 more in Usage →". */
  footer?: ReactNode;
  /** Mobile: no labels, smaller dots. */
  compact?: boolean;
} & HTMLAttributes<HTMLElement>;

const AXIS = ["00", "06", "12", "18", "now"];

/** Every agent's last 24 hours on Home (X68): one row per agent, dot size is turns in that hour. */
export function ActivityGrid({ rows, meta, summary, empty, footer, compact, className, ...rest }: ActivityGridProps) {
  const max = Math.max(1, ...rows.filter((r) => !r.total).flatMap((r) => r.hours));
  const single = rows.length === 1;
  const big = single ? 14 : compact ? 8 : 12;
  const dot = (v: number, scale: number) => (v <= 0 ? 2 : Math.max(4, Math.round((v / scale) * big)));
  const showLabels = !single && !compact && rows.length > 0;
  const cols = `${showLabels ? "88px " : ""}repeat(24, minmax(0, 1fr))`;
  // Dots that can be focused can't sit inside an image, so the grid becomes a labelled group.
  const hinted = rows.some((r) => r.hints?.length);
  return (
    <section className={cx("flex flex-col gap-12 rounded-xl border border-border bg-surface px-20 py-16", className)} {...rest}>
      <header className="flex items-baseline justify-between gap-12">
        <h3 className="type-ui-strong text-fg">Last 24 hours</h3>
        {meta && <span className="type-mono-xs text-faint">{meta}</span>}
      </header>
      <div
        role={hinted ? "group" : "img"}
        aria-label={
          rows.length
            ? `Turns per hour over the last 24 hours for ${rows.map((r) => r.label).join(", ")}`
            : "No activity in the last 24 hours"
        }
        className="grid items-center gap-y-4"
        style={{ gridTemplateColumns: cols }}
      >
        {(rows.length ? rows : [{ label: "", hours: Array(24).fill(0) } as ActivityRow]).map((r, ri) => {
          const scale = r.total ? Math.max(1, ...r.hours) : max;
          return (
            <div key={r.label || ri} className={cx("contents", r.total && "[&>*]:border-b [&>*]:border-border [&>*]:pb-4")}>
              {showLabels && (
                <span className={cx("truncate type-mono-xs", r.total ? "text-fg" : "text-faint")}>{r.label}</span>
              )}
              {r.hours.map((v, h) => {
                const hint = r.hints?.[h];
                return (
                <span
                  key={h}
                  title={hint}
                  aria-label={hint ? `${r.label ? `${r.label}, ` : ""}${hint}` : undefined}
                  role={hint ? "img" : undefined}
                  tabIndex={hint ? 0 : undefined}
                  className={cx("flex h-18 items-center justify-center rounded-xs", hint && "focus-visible:outline-2 focus-visible:outline-focus")}
                >
                  <motion.span
                    className={cx("rounded-pill", v <= 0 ? "bg-dot-off" : r.live && h === 23 ? "bg-live" : "bg-ink")}
                    style={{ width: dot(v, scale), height: dot(v, scale) }}
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    transition={{
                      duration: motionDurations.base,
                      ease: motionEases.arrive,
                      // Rows come in one after another, never a wave per dot.
                      delay: Math.min(ri * 0.03, 0.24),
                    }}
                  />
                </span>
                );
              })}
            </div>
          );
        })}
        {showLabels && <span />}
        {Array.from({ length: 24 }, (_, h) => {
          const i = [0, 6, 12, 18, 23].indexOf(h);
          return (
            <span key={h} className="pt-4 text-center type-mono-xs text-faint">
              {i >= 0 ? AXIS[i] : ""}
            </span>
          );
        })}
      </div>
      {rows.length === 0 && empty && (
        <div className="flex flex-col items-center gap-6 pb-4 text-center">
          <p className="type-body-medium text-fg">{empty.title}</p>
          <p className="type-ui text-muted">{empty.message}</p>
          {empty.action && <div className="pt-4">{empty.action}</div>}
        </div>
      )}
      {single && summary && <p className="type-ui text-muted">{summary}</p>}
      {footer && <div className="flex items-center justify-between gap-12">{footer}</div>}
    </section>
  );
}
