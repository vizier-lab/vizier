import type { ButtonHTMLAttributes } from "react";
import { motion } from "motion/react";
import { cx } from "../../lib/cx";
import { motionSprings } from "../../tokens/tokens";

export type SwitchProps = {
  checked: boolean;
  onCheckedChange?: (checked: boolean) => void;
  /** Required unless a visible `<label>` points at it. */
  "aria-label"?: string;
} & Omit<ButtonHTMLAttributes<HTMLButtonElement>, "onChange">;

/** An on/off switch that applies immediately (D18). */
export function Switch({ checked, onCheckedChange, onClick, className, type = "button", ...rest }: SwitchProps) {
  return (
    <button
      type={type}
      role="switch"
      aria-checked={checked}
      onClick={(e) => {
        onClick?.(e);
        if (!e.defaultPrevented) onCheckedChange?.(!checked);
      }}
      className={cx(
        "inline-flex h-18 w-32 shrink-0 items-center rounded-pill p-2 transition-colors disabled:opacity-40",
        checked ? "bg-ink enabled:hover:bg-ink-hover enabled:active:bg-ink-pressed" : "bg-border-strong enabled:hover:bg-border-hover",
        className,
      )}
      {...rest}
    >
      {/* The thumb travels the track's inner width (32 - 2*2 - 14) and springs to rest. */}
      <motion.span
        aria-hidden
        className={cx("size-14 rounded-pill transition-colors", checked ? "bg-ground" : "bg-muted")}
        animate={{ x: checked ? 14 : 0 }}
        transition={motionSprings.crisp}
      />
    </button>
  );
}
