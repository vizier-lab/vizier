import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { StatusDot, type StatusState } from "../Status/Status";

/**
 * What a calendar chip says about its moment: how the run ended, that it is still ahead,
 * that it passed with no record of how it went, or that its task is paused.
 */
export type ScheduleChipState = "answered" | "running" | "no-response" | "interrupted" | "upcoming" | "past" | "paused";

const DOT: Partial<Record<ScheduleChipState, StatusState>> = {
  answered: "answered",
  running: "running",
  "no-response": "no-response",
  interrupted: "interrupted",
  upcoming: "not-run",
};

const LOOK: Record<ScheduleChipState, string> = {
  answered: "bg-raised text-soft",
  running: "border border-live bg-raised text-soft",
  "no-response": "bg-retrying-bg text-soft",
  interrupted: "bg-raised text-soft",
  upcoming: "border border-border-strong text-muted",
  past: "bg-raised text-muted",
  paused: "border border-dashed border-border-strong text-faint",
};

export type ScheduleChipProps = {
  state: ScheduleChipState;
  /** "09:00". Left out for a task that runs several times a day. */
  time?: string;
  title: ReactNode;
  /** A task that runs several times a day: "4×". Replaces the dot and time with a repeat mark. */
  repeat?: string;
} & Omit<ButtonHTMLAttributes<HTMLButtonElement>, "title">;

/** One scheduled moment in a month cell or the day's list. */
export function ScheduleChip({ state, time, title, repeat, type = "button", className, ...rest }: ScheduleChipProps) {
  const dot = DOT[state];
  return (
    <button
      type={type}
      className={cx(
        "flex h-20 w-full min-w-0 items-center gap-6 rounded-xs px-6 text-left transition-colors enabled:hover:bg-pressed",
        repeat ? "bg-raised text-soft" : LOOK[state],
        className,
      )}
      {...rest}
    >
      {repeat ? (
        <Icon name="refresh" size={12} tone="muted" />
      ) : dot ? (
        <StatusDot state={dot} />
      ) : state === "past" ? (
        <span aria-hidden className="flex size-12 shrink-0 items-center justify-center">
          <span className="size-7 rounded-pill bg-unlit" />
        </span>
      ) : null}
      {time && !repeat && <span className="type-mono-xs shrink-0 text-muted">{time}</span>}
      <span className="min-w-0 flex-1 truncate type-small">{title}</span>
      {repeat && <span className="type-mono-xs shrink-0 text-faint">{repeat}</span>}
    </button>
  );
}
