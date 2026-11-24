import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

/** The space scale (`--vz-space-*`): the only gaps a layout may take. */
export type Gap = 0 | 2 | 4 | 6 | 8 | 10 | 12 | 14 | 16 | 20 | 24 | 32 | 48;

export const GAP: Record<Gap, string> = {
  0: "gap-0",
  2: "gap-2",
  4: "gap-4",
  6: "gap-6",
  8: "gap-8",
  10: "gap-10",
  12: "gap-12",
  14: "gap-14",
  16: "gap-16",
  20: "gap-20",
  24: "gap-24",
  32: "gap-32",
  48: "gap-48",
};

export type StackProps = {
  gap?: Gap;
  /** Cross-axis alignment (horizontal, in a stack). */
  align?: "start" | "center" | "end" | "stretch";
} & HTMLAttributes<HTMLDivElement>;

const ALIGN = {
  start: "items-start",
  center: "items-center",
  end: "items-end",
  stretch: "items-stretch",
};

/** Children in a column with one gap between them. */
export function Stack({ gap = 12, align = "stretch", className, ...rest }: StackProps) {
  return <div className={cx("flex min-w-0 flex-col", GAP[gap], ALIGN[align], className)} {...rest} />;
}
