import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type SetupStepCardProps = {
  step: number;
  state: "done" | "current" | "upcoming";
  title: ReactNode;
  description: ReactNode;
  /** "Manage" when done, "Create agent" when current. */
  action?: ReactNode;
} & Omit<HTMLAttributes<HTMLLIElement>, "title">;

/** One step of first-run setup on Home (X54). Render inside an <ol>. */
export function SetupStepCard({ step, state, title, description, action, className, ...rest }: SetupStepCardProps) {
  return (
    <li
      aria-current={state === "current" ? "step" : undefined}
      className={cx(
        "flex items-center gap-16 rounded-xl border bg-surface px-20 py-20",
        state === "current" ? "border-border-strong" : "border-border",
        className,
      )}
      {...rest}
    >
      <span
        aria-hidden
        className={cx(
          "flex size-30 shrink-0 items-center justify-center rounded-pill type-mono-xs-medium",
          state === "done" && "bg-raised text-fg",
          state === "current" && "bg-ink text-ground",
          state === "upcoming" && "border border-border-strong text-faint",
        )}
      >
        {state === "done" ? <Icon name="check" size={14} /> : step}
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-4">
        <h3 className={cx("type-body-strong", state === "upcoming" ? "text-muted" : "text-fg")}>
          {title}
          <span className="sr-only">, step {step}, {state === "done" ? "done" : state === "current" ? "next" : "later"}</span>
        </h3>
        <p className="max-w-420 type-small-para text-faint">{description}</p>
      </div>
      {action}
    </li>
  );
}
