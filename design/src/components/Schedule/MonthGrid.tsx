import type { ReactNode } from "react";
import { cx } from "../../lib/cx";

export type MonthGridDay = {
  /** Stable id, usually the ISO date. */
  key: string;
  /** The date number. */
  day: number;
  /** Shown in place of the number on the first of a month: "Oct 1". */
  label?: string;
  today?: boolean;
  /** A day of the neighbouring month, drawn quieter. */
  outside?: boolean;
  /** Names the day for assistive tech: "Friday 30 October". */
  name: string;
  /** The chips that fit, as ScheduleChips. */
  chips: ReactNode[];
  /** How many more there are than `chips` shows. */
  more?: number;
};

export type MonthGridProps = {
  /** Names the grid for assistive tech. */
  label: string;
  weekdays: string[];
  /** Rows of seven days. */
  weeks: MonthGridDay[][];
  onOpenDay?: (key: string) => void;
  onOpenMore?: (key: string, anchor: HTMLElement) => void;
};

/** A month as weeks of day cells with their chips (D52). The caller decides how many chips fit. */
export function MonthGrid({ label, weekdays, weeks, onOpenDay, onOpenMore }: MonthGridProps) {
  return (
    <div role="group" aria-label={label} className="overflow-hidden rounded-xl border border-border bg-surface">
      <div className="grid grid-cols-7 border-b border-border bg-sidebar">
        {weekdays.map((d) => (
          <span key={d} className="h-34 px-12 py-10 type-label uppercase text-faint">
            {d}
          </span>
        ))}
      </div>
      {weeks.map((week, wi) => (
        <div key={week[0]?.key ?? wi} className={cx("grid grid-cols-7", wi < weeks.length - 1 && "border-b border-border")}>
          {week.map((d, di) => (
            <div
              key={d.key}
              className={cx("flex min-h-128 min-w-0 flex-col gap-4 p-6", di < 6 && "border-r border-border", d.outside && "bg-sidebar")}
            >
              <button
                type="button"
                aria-label={d.label ? `${d.label}, ${d.name}` : d.name}
                aria-current={d.today ? "date" : undefined}
                onClick={() => onOpenDay?.(d.key)}
                className={cx(
                  "type-mono-xs flex h-22 min-w-22 items-center justify-center self-start rounded-pill px-6 transition-colors",
                  d.today ? "bg-ink text-ground" : cx("hover:bg-raised", d.outside ? "text-faint" : "text-muted"),
                )}
              >
                {d.label ?? d.day}
              </button>
              {d.chips}
              {d.more ? (
                <button
                  type="button"
                  onClick={(e) => onOpenMore?.(d.key, e.currentTarget)}
                  className="self-start rounded-xs px-6 type-small-medium text-muted hover:text-fg"
                >
                  +{d.more} more
                </button>
              ) : null}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}
