import { useId, type InputHTMLAttributes, type ReactNode } from "react";
import { cx } from "../../lib/cx";

export type SliderProps = {
  label?: ReactNode;
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  /** Shown beside the label; the value by default. */
  display?: ReactNode;
  /** Required when there's no visible label. */
  "aria-label"?: string;
} & Omit<InputHTMLAttributes<HTMLInputElement>, "value" | "onChange" | "min" | "max" | "step" | "type">;

/**
 * A range input for a number between two bounds. Keyboard (arrows, Home, End, Page keys) and
 * ARIA come from the native control; it applies as the thumb moves.
 */
export function Slider({ label, value, onChange, min = 0, max = 100, step = 1, display, id, className, disabled, ...rest }: SliderProps) {
  const auto = useId();
  const inputId = id ?? auto;
  return (
    <div className={cx("flex min-w-0 flex-col gap-6", className)}>
      {(label || display !== null) && (
        <div className="flex items-baseline justify-between gap-12">
          {label && (
            <label htmlFor={inputId} className="type-ui-medium text-soft">
              {label}
            </label>
          )}
          <span className="type-mono-sm text-muted">{display ?? value}</span>
        </div>
      )}
      <input
        id={inputId}
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value))}
        className="h-20 w-full cursor-pointer accent-ink disabled:cursor-not-allowed disabled:opacity-40"
        {...rest}
      />
    </div>
  );
}
