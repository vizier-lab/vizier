import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type TrailHeadProps = {
  /** Live while the turn works; done once it answered; failed when it ended in an error. */
  state: "live" | "done" | "failed";
  /** "Working 6.4s", "Worked 18.4s", "Failed after 12.0s". */
  title: ReactNode;
  /** "3 python runs · 4 passages". */
  counts?: ReactNode;
  /** The trail is open below. Collapsed shows ›, open shows ⌄. */
  expanded?: boolean;
} & Omit<ButtonHTMLAttributes<HTMLButtonElement>, "title">;

/** The summary line of an activity trail (D50); it opens and closes the trail. */
export function TrailHead({ state, title, counts, expanded = true, type = "button", className, ...rest }: TrailHeadProps) {
  return (
    <button
      type={type}
      aria-expanded={expanded}
      className={cx("flex w-full items-center gap-8 rounded-xs text-left", className)}
      {...rest}
    >
      {state !== "done" && <span aria-hidden className={cx("size-6 shrink-0 rounded-pill", state === "live" ? "bg-live" : "bg-failed")} />}
      <span className={cx("type-small-medium", state === "failed" ? "text-failed-text" : "text-fg")}>{title}</span>
      {counts && <span className="min-w-0 flex-1 truncate type-mono-xs text-faint">{counts}</span>}
      <Icon name={expanded ? "chevron-down" : "chevron-right"} size={14} tone="faint" className="ml-auto" />
    </button>
  );
}
