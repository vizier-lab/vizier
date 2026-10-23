import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type TopicHeaderProps = {
  topic: string;
  /** "webui · topic 3 of 5 · started 11:40". */
  meta?: ReactNode;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** The topic's name and meta (X42). It opens the topic Picker: pass it as its trigger. */
export function TopicHeader({ topic, meta, type = "button", className, ...rest }: TopicHeaderProps) {
  return (
    <button type={type} className={cx("flex flex-col items-start gap-2 rounded-sm text-left", className)} {...rest}>
      <span className="flex items-center gap-8">
        <span className="type-heading text-fg">{topic}</span>
        <Icon name="chevron-down" size={14} tone="muted" />
      </span>
      {meta && <span className="type-mono-xs text-faint">{meta}</span>}
    </button>
  );
}
