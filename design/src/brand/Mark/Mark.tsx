import type { SVGProps } from "react";
import { cx } from "../../lib/cx";
import { MARK_LIT, MARK_STAR, MARK_UNLIT } from "./geometry";

export type MarkProps = {
  /** Live turns the top-right dot emerald. Static surfaces show idle. */
  state?: "idle" | "live";
  /** Shows the unlit dots of the 8×8 grid. */
  grid?: boolean;
  /** Width and height in px. */
  size?: number;
  /** Accessible name; defaults to "Vizier". Pass "" when a wordmark sits beside it. */
  title?: string;
} & Omit<SVGProps<SVGSVGElement>, "children">;

// 8 cells of 20 units: lit dots are 12.5 wide, grid dots 5.
const PITCH = 20;
const DOT = 6.25;
const GRID_DOT = 2.5;
const at = (i: number) => i * PITCH + PITCH / 2;

/** The Vizier mark (B1). */
export function Mark({ state = "idle", grid = false, size = 32, title = "Vizier", className, ...rest }: MarkProps) {
  const labelled = title !== "";
  return (
    <svg
      viewBox="0 0 160 160"
      width={size}
      height={size}
      role={labelled ? "img" : undefined}
      aria-label={labelled ? title : undefined}
      aria-hidden={labelled ? undefined : true}
      className={cx("shrink-0", className)}
      {...rest}
    >
      {grid && (
        <g className="fill-unlit">
          {MARK_UNLIT.map(([x, y]) => (
            <circle key={`${x}-${y}`} cx={at(x)} cy={at(y)} r={GRID_DOT} />
          ))}
        </g>
      )}
      <g className="fill-ink">
        {MARK_LIT.map(([x, y]) => (
          <circle key={`${x}-${y}`} cx={at(x)} cy={at(y)} r={DOT} />
        ))}
      </g>
      <circle
        cx={at(MARK_STAR[0])}
        cy={at(MARK_STAR[1])}
        r={DOT}
        className={state === "live" ? "fill-live" : "fill-ink"}
      />
    </svg>
  );
}
