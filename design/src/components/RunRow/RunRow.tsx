import type { ReactNode } from "react";
import { Status, type StatusState } from "../Status/Status";
import { ListItem, type ListItemProps } from "../ListItem/ListItem";

export type RunRowProps = {
  /** "Oct 20, 09:00". */
  time: ReactNode;
  state: Extract<StatusState, "answered" | "running" | "no-response" | "not-run" | "interrupted">;
  /** "41s", "1m 02s"; "–" when there is none. */
  duration?: ReactNode;
} & Omit<ListItemProps, "title" | "subtitle" | "meta" | "leading" | "trailing">;

/** One run of a task (X28): when, how it ended, how long. Built on ListItem. */
export function RunRow({ time, state, duration, ...rest }: RunRowProps) {
  return (
    <ListItem
      leading={<span className="w-100 type-mono-sm text-fg">{time}</span>}
      title={<Status state={state} font="sans" />}
      meta={duration ?? "–"}
      {...rest}
    />
  );
}
