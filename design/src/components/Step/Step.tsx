import type { LiHTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type StepState = "done" | "current" | "upcoming" | "error";

export type StepProps = {
  number: number;
  label: ReactNode;
  /** Error: a field in this step needs fixing. */
  state?: StepState;
} & LiHTMLAttributes<HTMLLIElement>;

const MARK: Record<StepState, string> = {
  done: "bg-raised text-fg",
  current: "bg-ink text-ground",
  upcoming: "border border-border-strong text-faint",
  error: "border border-failed-line bg-failed-bg text-failed-text",
};

const LABEL: Record<StepState, string> = {
  done: "type-ui text-fg",
  current: "type-ui-medium text-fg",
  upcoming: "type-ui text-faint",
  error: "type-ui-medium text-failed-text",
};

const SPOKEN: Record<StepState, string> = {
  done: "done",
  current: "current step",
  upcoming: "not started",
  error: "needs fixing",
};

/** One step of a Stepper (D9). Render inside an `<ol>`. */
export function Step({ number, label, state = "upcoming", className, ...rest }: StepProps) {
  return (
    <li
      aria-current={state === "current" ? "step" : undefined}
      className={cx("flex items-center gap-8", className)}
      {...rest}
    >
      <span aria-hidden className={cx("flex size-22 shrink-0 items-center justify-center rounded-pill type-mono-xs-medium", MARK[state])}>
        {state === "done" ? <Icon name="check" size={12} /> : number}
      </span>
      <span className={cx("whitespace-nowrap", LABEL[state])}>
        {label}
        <span className="sr-only">, step {number}, {SPOKEN[state]}</span>
      </span>
    </li>
  );
}
