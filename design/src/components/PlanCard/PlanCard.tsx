import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";
import { Button } from "../Button/Button";

export type PlanCardProps = {
  title: string;
  summary?: string;
  steps: string[];
  /**
   * The one action. Absent once a later message exists: the card is then history, and a plan is
   * rejected or revised by typing a new message.
   */
  onBuild?: () => void;
  /** Build was pressed and the chat is switching mode. */
  building?: boolean;
} & Omit<HTMLAttributes<HTMLElement>, "title">;

/**
 * A plan the agent proposed in Plan mode, drawn inside its reply. Numbered steps in mono so the
 * order reads at a glance; one primary action, which switches the chat to Build and asks the agent
 * to carry the plan out.
 */
export function PlanCard({ title, summary, steps, onBuild, building, className, ...rest }: PlanCardProps) {
  return (
    <section aria-label={`Plan: ${title}`} className={cx("flex flex-col gap-12 rounded-lg border border-border-strong bg-surface px-16 py-14", className)} {...rest}>
      <header className="flex flex-col gap-2">
        <span className="type-label uppercase text-faint">Plan</span>
        <h3 className="type-ui-strong text-fg">{title}</h3>
        {summary && <p className="type-small text-muted">{summary}</p>}
      </header>
      <ol className="flex flex-col gap-6">
        {steps.map((step, i) => (
          <li key={i} className="flex items-baseline gap-10">
            <span aria-hidden className="w-16 shrink-0 text-right type-mono-xs text-faint">
              {i + 1}
            </span>
            <span className="min-w-0 type-small text-soft">{step}</span>
          </li>
        ))}
      </ol>
      {onBuild && (
        <div>
          <Button variant="primary" size="sm" icon="play" loading={building} onClick={onBuild}>
            {building ? "Switching to Build…" : "Build this plan"}
          </Button>
        </div>
      )}
    </section>
  );
}
