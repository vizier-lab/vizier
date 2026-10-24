import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";
import { GAP, type Gap } from "../Stack/Stack";

export type RowProps = {
  gap?: Gap;
  /** Cross-axis alignment (vertical, in a row). */
  align?: "start" | "center" | "end" | "stretch";
  justify?: "start" | "center" | "end" | "between";
  /** Let children drop to the next line instead of overflowing. */
  wrap?: boolean;
} & HTMLAttributes<HTMLDivElement>;

const ALIGN = {
  start: "items-start",
  center: "items-center",
  end: "items-end",
  stretch: "items-stretch",
};

const JUSTIFY = {
  start: "justify-start",
  center: "justify-center",
  end: "justify-end",
  between: "justify-between",
};

/** Children in a line with one gap between them. */
export function Row({ gap = 12, align = "center", justify = "start", wrap = false, className, ...rest }: RowProps) {
  return <div className={cx("flex min-w-0", wrap && "flex-wrap", GAP[gap], ALIGN[align], JUSTIFY[justify], className)} {...rest} />;
}
