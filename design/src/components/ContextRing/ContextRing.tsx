import type { HTMLAttributes } from "react";
import { motion } from "motion/react";
import { cx } from "../../lib/cx";
import { motionDurations, motionEases } from "../../tokens/tokens";
import { Tooltip } from "../Tooltip/Tooltip";

export type ContextRingProps = {
  /** Tokens in the conversation's context now. */
  used: number;
  /** The model's context window. */
  window: number;
  /** "claude-sonnet-5-5 · checkpoints automatically". */
  detail?: string;
} & HTMLAttributes<HTMLSpanElement>;

const compact = (n: number) => (n >= 1000 ? `${Math.round(n / 1000)}k` : String(n));

/**
 * How full the model's context is (X36): a small ring beside Send, in the
 * foreground colour only. Hover or focus shows the exact numbers.
 */
export function ContextRing({ used, window, detail, className, ...rest }: ContextRingProps) {
  const pct = window > 0 ? Math.min(1, used / window) : 0;
  const r = 7;
  const c = 2 * Math.PI * r;
  const label = `${compact(used)} of ${compact(window)} tokens`;
  return (
    <Tooltip label={label} detail={detail} side="top">
      <span
        role="meter"
        tabIndex={0}
        aria-label="Context used"
        aria-valuemin={0}
        aria-valuemax={window}
        aria-valuenow={used}
        aria-valuetext={label}
        className={cx("inline-flex size-18 shrink-0 rounded-pill", className)}
        {...rest}
      >
        <svg viewBox="0 0 18 18" width={18} height={18} aria-hidden className="-rotate-90">
          <circle cx="9" cy="9" r={r} fill="none" strokeWidth="2.5" className="stroke-border-strong" />
          {/* The arc is drawn full-circle and uncovered by its offset, so it sweeps on. */}
          <motion.circle
            cx="9"
            cy="9"
            r={r}
            fill="none"
            strokeWidth="2.5"
            strokeDasharray={c}
            initial={{ strokeDashoffset: c }}
            animate={{ strokeDashoffset: c * (1 - pct) }}
            transition={{ duration: motionDurations.slow, ease: motionEases.arrive }}
            className="stroke-fg"
          />
        </svg>
      </span>
    </Tooltip>
  );
}
