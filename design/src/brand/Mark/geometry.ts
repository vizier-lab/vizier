/**
 * The mark: a bold V on an 8×8 dot grid, strokes two dots thick, 28 lit dots
 * all the same size. The top-right dot is the only one that changes: it
 * turns live when the thing it stands for is live.
 */
export const MARK_ROWS = [
  "##....#*",
  "##....##",
  ".##..##.",
  ".##..##.",
  "..####..",
  "..####..",
  "...##...",
  "...##...",
] as const;

/** One mark cell, as [column, row]. */
export type Cell = readonly [number, number];

function cells(match: (ch: string) => boolean): Cell[] {
  return MARK_ROWS.flatMap((row, y) => [...row].flatMap((ch, x) => (match(ch) ? [[x, y] as const] : [])));
}

/** The 27 dots that never change. */
export const MARK_LIT = cells((ch) => ch === "#");
/** The signal dot (top right). */
export const MARK_STAR: Cell = cells((ch) => ch === "*")[0];
/** The 36 unlit positions, drawn when the grid is shown. */
export const MARK_UNLIT = cells((ch) => ch === ".");
