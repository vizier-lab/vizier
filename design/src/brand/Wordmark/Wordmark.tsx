import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

export type WordmarkProps = {
  /** Live turns the second i-dot emerald. One live signal per lockup. */
  state?: "idle" | "live";
  /** Font size in px; the drawn wordmark is 100. */
  size?: number;
} & HTMLAttributes<HTMLSpanElement>;

/** A dotless i with a round dot; the dot sits 0.254em from the top, 0.155em wide. */
function I({ live }: { live?: boolean }) {
  return (
    <span className="relative inline-block">
      ı
      <span
        aria-hidden
        className={cx("absolute left-1/2 rounded-pill", live ? "bg-live" : "bg-ink")}
        style={{ width: "0.155em", height: "0.155em", top: "0.254em", transform: "translateX(-50%)" }}
      />
    </span>
  );
}

/**
 * The "star-dot" wordmark (B2): lowercase vizier in Schibsted Grotesk Bold with
 * round i-dots. Read by assistive tech as "vizier".
 */
export function Wordmark({ state = "idle", size = 32, className, style, ...rest }: WordmarkProps) {
  return (
    <span
      role="img"
      aria-label="vizier"
      className={cx("inline-flex font-sans font-bold text-ink select-none", className)}
      style={{ fontSize: size, lineHeight: 1.23, ...style }}
      {...rest}
    >
      <span aria-hidden className="contents">
        v<I />z<I live={state === "live"} />er
      </span>
    </span>
  );
}
