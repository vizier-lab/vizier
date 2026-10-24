import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type TrailHeadProps = {
  /** Live while the turn works; done once it answered; failed when it ended in an error. */
  state: "live" | "done" | "failed";
  /** "Worked 18.4s", "Failed after 12.0s"; while live, a line like "Archivist is gathering thoughts" (the three wave dots follow it). */
  title: ReactNode;
  /** "3 python runs · 4 passages". */
  counts?: ReactNode;
  /** The trail is open below. Collapsed shows ›, open shows ⌄. */
  expanded?: boolean;
  /** Nothing to open yet: no chevron, and the line isn't a button. */
  plain?: boolean;
} & Omit<ButtonHTMLAttributes<HTMLButtonElement>, "title">;

/** The summary line of an activity trail (D50); it opens and closes the trail. */
export function TrailHead({ state, title, counts, expanded = true, plain, type = "button", className, ...rest }: TrailHeadProps) {
  return (
    <button
      type={type}
      aria-expanded={plain ? undefined : expanded}
      disabled={plain}
      className={cx("flex w-full items-center gap-8 rounded-xs text-left", className)}
      {...rest}
    >
      {state === "failed" && <span aria-hidden className="size-6 shrink-0 rounded-pill bg-failed" />}
      <span className={cx("type-small-medium", state === "failed" ? "text-failed-text" : "text-fg")}>{title}</span>
      {state === "live" && (
        // The same three dots as Thinking: two quiet, the last live.
        <span aria-hidden className="flex shrink-0 gap-4">
          {["bg-muted", "bg-muted", "bg-live"].map((c, i) => (
            <span key={i} className={cx("size-5 rounded-pill animate-dot-wave motion-reduce:animate-none", c)} style={{ animationDelay: `${i * 0.2}s` }} />
          ))}
        </span>
      )}
      {counts && <span className="min-w-0 flex-1 truncate type-mono-xs text-faint">{counts}</span>}
      {!plain && <Icon name={expanded ? "chevron-down" : "chevron-right"} size={14} tone="faint" className="ml-auto" />}
    </button>
  );
}
