import type { HTMLAttributes } from "react";

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
export function MiddleTruncate({ text, max = 40, ...rest }: MiddleTruncateProps) {
  const short = middleTruncate(text, max);
  if (short === text) return <span {...rest}>{text}</span>;
  return (
    <span title={text} {...rest}>
      <span aria-hidden>{short}</span>
      <span className="sr-only">{text}</span>
    </span>
  );
}
