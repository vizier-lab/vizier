import type { CSSProperties, HTMLAttributes } from "react";
import { cx } from "../../lib/cx";
import { GAP, type Gap } from "../Stack/Stack";

const COLS: Record<number, string> = {
  1: "grid-cols-1",
  2: "grid-cols-2",
  3: "grid-cols-3",
  4: "grid-cols-4",
  5: "grid-cols-5",
  6: "grid-cols-6",
  7: "grid-cols-7",
  8: "grid-cols-8",
  9: "grid-cols-9",
  10: "grid-cols-10",
  11: "grid-cols-11",
  12: "grid-cols-12",
};

export type GridProps = {
  /** 1–12 equal columns. */
  cols?: number;
  gap?: Gap;
  /**
   * Smallest column width in px. When set, `cols` is the most columns and the grid drops to
   * fewer as the space narrows, so a dashboard reads on a phone.
   */
  minCol?: number;
} & HTMLAttributes<HTMLDivElement>;

/** Children in equal columns. */
export function Grid({ cols = 2, gap = 12, minCol, className, style, ...rest }: GridProps) {
  const n = Math.max(1, Math.min(12, Math.round(cols)));
  // With a minimum width the columns are as many as fit, up to `cols`: the track is at least
  // the larger of `minCol` and an equal share of the row.
  const responsive: CSSProperties | undefined = minCol
    ? { gridTemplateColumns: `repeat(auto-fit, minmax(max(${minCol}px, calc((100% - ${(n - 1) * 48}px) / ${n})), 1fr))` }
    : undefined;
  return <div className={cx("grid min-w-0", !minCol && COLS[n], GAP[gap], className)} style={{ ...responsive, ...style }} {...rest} />;
}
