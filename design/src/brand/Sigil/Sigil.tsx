import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";
import { sigilCells, sigilSignalCell } from "./sigilCells";

export type SigilSize = "sm" | "md" | "lg" | "xl";

export type SigilProps = {
  /** The agent id the pattern is drawn from. */
  agentId: string;
  /** sm 22 (chat), md 34 (switcher, cards), lg 64 (new agent preview), xl 86 (config identity). */
  size?: SigilSize;
  /** Turns the signal dot emerald while the agent is live. */
  live?: boolean;
  /** Accessible name, e.g. the agent's name. Decorative when omitted. */
  label?: string;
} & HTMLAttributes<HTMLSpanElement>;

// Outer box, inner grid and corner radius in px, from the drawn Sm and Md.
const SIZES: Record<SigilSize, { box: number; grid: number; radius: number }> = {
  sm: { box: 22, grid: 16, radius: 6 },
  md: { box: 34, grid: 25, radius: 7.5 },
  lg: { box: 64, grid: 47, radius: 14 },
  xl: { box: 86, grid: 63, radius: 19 },
};

/** An agent's 5×5 identicon (B3). */
export function Sigil({ agentId, size = "md", live = false, label, className, style, ...rest }: SigilProps) {
  const cells = sigilCells(agentId);
  const signal = live ? sigilSignalCell(cells) : -1;
  const { box, grid, radius } = SIZES[size];
  return (
    <span
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      className={cx("inline-flex shrink-0 items-center justify-center border border-border-strong bg-ground", className)}
      style={{ width: box, height: box, borderRadius: radius, ...style }}
      {...rest}
    >
      {/* 5 cells of 3 units plus a 1-unit margin; dots are 2 units wide, as drawn. */}
      <svg viewBox="0 0 16 16" width={grid} height={grid} aria-hidden>
        {cells.map((lit, i) =>
          lit ? (
            <circle
              key={i}
              cx={(i % 5) * 3 + 2}
              cy={Math.floor(i / 5) * 3 + 2}
              r={1}
              className={i === signal ? "fill-live" : "fill-ink"}
            />
          ) : null,
        )}
      </svg>
    </span>
  );
}
