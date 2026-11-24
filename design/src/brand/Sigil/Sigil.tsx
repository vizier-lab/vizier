import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";
import { sigilCells, sigilSignalCell } from "./sigilCells";

export type SigilSize = "sm" | "md" | "rail" | "lg" | "xl";

export type SigilProps = {
  /** The agent id the pattern is drawn from. */
  agentId: string;
  /** sm 22 (chat), md 34 (switcher, cards), rail 42 (the app rail), lg 64 (new agent preview), xl 86 (config identity). */
  size?: SigilSize;
  /** Turns the signal dot emerald while the agent is live. */
  live?: boolean;
  /** A picture the agent was given: drawn in place of the pattern, so there is no signal dot to show it live. */
  image?: string;
  /** Accessible name, e.g. the agent's name. Decorative when omitted. */
  label?: string;
} & HTMLAttributes<HTMLSpanElement>;

// Outer box, inner grid and corner radius in px, from the drawn Sm and Md.
const SIZES: Record<SigilSize, { box: number; grid: number; radius: number }> = {
  sm: { box: 22, grid: 16, radius: 6 },
  md: { box: 34, grid: 25, radius: 7.5 },
  rail: { box: 42, grid: 31, radius: 9 },
  lg: { box: 64, grid: 47, radius: 14 },
  xl: { box: 86, grid: 63, radius: 19 },
};

/** An agent's 8×8 identicon (B3). */
export function Sigil({ agentId, size = "md", live = false, image, label, className, style, ...rest }: SigilProps) {
  const cells = sigilCells(agentId);
  const signal = live ? sigilSignalCell(cells) : -1;
  const { box, grid, radius } = SIZES[size];
  return (
    <span
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      className={cx("inline-flex shrink-0 items-center justify-center overflow-hidden border border-border-strong bg-ground", className)}
      style={{ width: box, height: box, borderRadius: radius, ...style }}
      {...rest}
    >
      {image ? (
        <img src={image} alt="" draggable={false} className="size-full object-cover" />
      ) : (
      <>
      {/* 8 cells of 2 units; dots are 1.4 units wide. */}
      <svg viewBox="0 0 16 16" width={grid} height={grid} aria-hidden>
        {cells.map((lit, i) =>
          lit ? (
            <circle
              key={i}
              cx={(i % 8) * 2 + 1}
              cy={Math.floor(i / 8) * 2 + 1}
              r={0.7}
              className={i === signal ? "fill-live" : "fill-ink"}
            />
          ) : null,
        )}
      </svg>
      </>
      )}
    </span>
  );
}
