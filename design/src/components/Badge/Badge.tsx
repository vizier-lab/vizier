import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

export type BadgeTone = "neutral" | "rank" | "live" | "failed" | "retrying";

export type BadgeProps = {
  /** Tone carries meaning: rank is lapis, live emerald, failed ember, retrying amber. */
  tone?: BadgeTone;
} & HTMLAttributes<HTMLSpanElement>;

const TONES: Record<BadgeTone, string> = {
  neutral: "bg-raised text-muted",
  rank: "bg-rank-bg text-rank-text",
  live: "bg-raised text-live-text",
  failed: "bg-failed-bg text-failed-text",
  retrying: "bg-retrying-bg text-retrying",
};

/** A small mono tag (D23): roles, tags and keywords, `built-in`, `granted`, versions. */
export function Badge({ tone = "neutral", className, ...rest }: BadgeProps) {
  return (
    <span
      className={cx("inline-flex h-16 shrink-0 items-center rounded-xs px-4 type-mono-xs-medium whitespace-nowrap", TONES[tone], className)}
      {...rest}
    />
  );
}
