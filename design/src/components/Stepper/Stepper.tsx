import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Step, type StepState } from "../Step/Step";

export type StepperProps = {
  steps: { label: ReactNode; state?: StepState }[];
  /** Header: steps joined by rules (New agent). Compact: steps only (Onboarding). */
  look?: "header" | "compact";
  "aria-label"?: string;
} & HTMLAttributes<HTMLOListElement>;

/** A row of Steps (X10). The current one carries aria-current. */
export function Stepper({ steps, look = "header", className, "aria-label": label = "Progress", ...rest }: StepperProps) {
  return (
    <ol aria-label={label} className={cx("flex items-center", look === "header" ? "gap-10" : "gap-16", className)} {...rest}>
      {steps.map((s, i) => (
        <Step
          key={i}
          number={i + 1}
          label={s.label}
          state={s.state}
          className={i > 0 && look === "header" ? "before:mr-2 before:h-px before:w-24 before:bg-border-strong before:content-['']" : undefined}
        />
      ))}
    </ol>
  );
}
