import type { HTMLAttributes } from "react";
import { cx } from "./cx";

/**
 * Shortens `text` to about `max` characters by cutting the middle, keeping the
 * start and the end (the file name): "projects/2025-archive/…/q4-board-notes.md".
 */
export function middleTruncate(text: string, max: number) {
  if (text.length <= max) return text;
  const keep = max - 1;
  const tail = Math.ceil(keep * 0.6);
  return `${text.slice(0, keep - tail)}…${text.slice(text.length - tail)}`;
}

export type MiddleTruncateProps = { text: string; max?: number } & HTMLAttributes<HTMLSpanElement>;

/** Long names and paths, cut in the middle; the full value shows on hover and is what gets copied. */
export function MiddleTruncate({ text, max = 40, className, ...rest }: MiddleTruncateProps) {
  const short = middleTruncate(text, max);
  if (short === text) return <span {...rest}>{text}</span>;
  // `relative` contains the sr-only copy: unpositioned, it sits after the uncut text and widens the page.
  return (
    <span title={text} className={cx("relative", className)} {...rest}>
      <span aria-hidden>{short}</span>
      <span className="sr-only">{text}</span>
    </span>
  );
}
