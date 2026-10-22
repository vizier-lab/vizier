import type { ButtonHTMLAttributes } from "react";
import { cx } from "../../lib/cx";

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
        "inline-flex h-18 w-32 shrink-0 items-center rounded-pill p-2 disabled:opacity-40",
        checked
          ? "justify-end bg-ink enabled:hover:bg-ink-hover enabled:active:bg-ink-pressed"
          : "justify-start bg-border-strong enabled:hover:bg-border-hover",
        className,
      )}
      {...rest}
    >
      <span aria-hidden className={cx("size-14 rounded-pill", checked ? "bg-ground" : "bg-muted")} />
    </button>
  );
}
