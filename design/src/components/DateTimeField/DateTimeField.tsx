import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { FieldFrame, fieldBox } from "../TextField/Field";
import { WEEKDAYS, addDays, addMonths, formatDateTime, monthGrid, sameDay, startOfDay, withTime } from "./calendar";

export type DateTimeFieldProps = {
  label?: ReactNode;
  helper?: ReactNode;
  value: Date | null;
  onChange: (value: Date | null) => void;
  /** Earlier days and times can't be picked. */
  min?: Date;
  invalid?: boolean;
  disabled?: boolean;
  placeholder?: string;
  id?: string;
  className?: string;
};

const two = (n: number) => String(n).padStart(2, "0");

/**
 * A date and a time in one field (X77): a button that opens a month grid and a time row.
 * Arrow keys move the day, PgUp/PgDn the month, Enter picks, Esc closes.
 */
export function DateTimeField({ label, helper, value, onChange, min, invalid, disabled, placeholder = "Pick a date and time", id, className }: DateTimeFieldProps) {
  const auto = useId();
  const base = id ?? auto;
  const [open, setOpen] = useState(false);
  const [cursor, setCursor] = useState<Date>(() => value ?? min ?? new Date());
  const root = useRef<HTMLDivElement>(null);
  const grid = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open) setCursor(value ?? min ?? new Date());
  }, [open]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", onDown);
    return () => document.removeEventListener("pointerdown", onDown);
  }, [open]);

  useEffect(() => {
    if (open) grid.current?.querySelector<HTMLButtonElement>('[tabindex="0"]')?.focus();
  }, [open, cursor]);

  const days = useMemo(() => monthGrid(cursor), [cursor]);
  const blocked = (d: Date) => !!min && startOfDay(d) < startOfDay(min);
  const time = value ?? min ?? new Date(cursor.getFullYear(), cursor.getMonth(), cursor.getDate(), 9, 0);

  const clampToMin = (d: Date) => (min && d < min ? new Date(min) : d);
  const pick = (day: Date) => {
    onChange(clampToMin(withTime(day, time)));
    setOpen(false);
  };
  const setTime = (h: number, m: number) => {
    const base = value ?? cursor;
    onChange(clampToMin(new Date(base.getFullYear(), base.getMonth(), base.getDate(), h, m)));
  };

  const onKey = (e: KeyboardEvent) => {
    const moves: Record<string, () => Date> = {
      ArrowLeft: () => addDays(cursor, -1),
      ArrowRight: () => addDays(cursor, 1),
      ArrowUp: () => addDays(cursor, -7),
      ArrowDown: () => addDays(cursor, 7),
      PageUp: () => addMonths(cursor, -1),
      PageDown: () => addMonths(cursor, 1),
      Home: () => addDays(cursor, -((cursor.getDay() + 6) % 7)),
      End: () => addDays(cursor, 6 - ((cursor.getDay() + 6) % 7)),
    };
    if (moves[e.key]) {
      e.preventDefault();
      setCursor(moves[e.key]());
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      if (!blocked(cursor)) pick(cursor);
    } else if (e.key === "Escape") {
      e.preventDefault();
      setOpen(false);
      root.current?.querySelector<HTMLButtonElement>("button")?.focus();
    }
  };

  const today = new Date();
  return (
    <FieldFrame label={label} helper={helper} invalid={invalid} ids={{ input: base, helper: `${base}-helper` }} className={className}>
      <div ref={root} className="relative">
        <button
          id={base}
          type="button"
          disabled={disabled}
          aria-haspopup="dialog"
          aria-expanded={open}
          aria-invalid={invalid || undefined}
          aria-describedby={helper ? `${base}-helper` : undefined}
          onClick={() => setOpen((o) => !o)}
          className={cx(fieldBox({ invalid, disabled }), "h-40 gap-8 pr-10 pl-12 text-left type-body")}
        >
          <span className={cx("min-w-0 flex-1 truncate", !value && "text-faint")}>{value ? formatDateTime(value) : placeholder}</span>
          <Icon name="calendar" size={15} tone="faint" />
        </button>
        {open && (
          <div role="dialog" aria-label="Choose date and time" onKeyDown={onKey} className="absolute top-full left-0 z-30 mt-6 flex w-296 flex-col gap-10 rounded-xl border border-border-strong bg-surface p-12 shadow-popover">
            <div className="flex items-center justify-between">
              <button type="button" aria-label="Previous month" onClick={() => setCursor(addMonths(cursor, -1))} className="flex size-28 items-center justify-center rounded-sm text-muted hover:bg-raised hover:text-fg">
                <Icon name="chevron-left" size={14} />
              </button>
              <span aria-live="polite" className="type-ui-strong text-fg">
                {cursor.toLocaleDateString("en-US", { month: "long", year: "numeric" })}
              </span>
              <button type="button" aria-label="Next month" onClick={() => setCursor(addMonths(cursor, 1))} className="flex size-28 items-center justify-center rounded-sm text-muted hover:bg-raised hover:text-fg">
                <Icon name="chevron-right" size={14} />
              </button>
            </div>
            <div ref={grid} role="group" aria-label="Days" className="grid grid-cols-7 gap-2">
              {WEEKDAYS.map((w) => (
                <span key={w} aria-hidden className="py-4 text-center type-mono-xs text-faint">
                  {w}
                </span>
              ))}
              {days.map((d) => {
                const inMonth = d.getMonth() === cursor.getMonth();
                const selected = !!value && sameDay(d, value);
                const off = blocked(d);
                return (
                  <button
                    key={d.toISOString()}
                    type="button"
                    tabIndex={sameDay(d, cursor) ? 0 : -1}
                    aria-pressed={selected}
                    aria-current={sameDay(d, today) ? "date" : undefined}
                    aria-label={d.toLocaleDateString("en-US", { weekday: "long", month: "long", day: "numeric", year: "numeric" })}
                    disabled={off}
                    onClick={() => pick(d)}
                    className={cx(
                      "flex size-34 items-center justify-center rounded-sm type-small",
                      selected ? "bg-ink text-ground" : inMonth ? "text-fg enabled:hover:bg-raised" : "text-faint enabled:hover:bg-raised",
                      sameDay(d, today) && !selected && "border border-border-strong",
                      off && "opacity-30",
                    )}
                  >
                    {d.getDate()}
                  </button>
                );
              })}
            </div>
            <div className="flex items-center gap-8 border-t border-border pt-10">
              <span className="type-label uppercase text-faint">Time</span>
              <input
                aria-label="Hour"
                inputMode="numeric"
                value={two(time.getHours())}
                onChange={(e) => setTime(Math.min(23, Math.max(0, Number(e.target.value.replace(/\D/g, "")) || 0)), time.getMinutes())}
                className="h-28 w-40 rounded-sm border border-border-strong bg-ground text-center type-mono-sm text-fg outline-none focus:border-live"
              />
              <span className="type-mono-sm text-faint">:</span>
              <input
                aria-label="Minute"
                inputMode="numeric"
                value={two(time.getMinutes())}
                onChange={(e) => setTime(time.getHours(), Math.min(59, Math.max(0, Number(e.target.value.replace(/\D/g, "")) || 0)))}
                className="h-28 w-40 rounded-sm border border-border-strong bg-ground text-center type-mono-sm text-fg outline-none focus:border-live"
              />
              <span className="flex-1" />
              {value && (
                <button type="button" onClick={() => (onChange(null), setOpen(false))} className="type-small text-muted hover:text-fg">
                  Clear
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    </FieldFrame>
  );
}
