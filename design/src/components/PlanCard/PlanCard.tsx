import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Button } from "../Button/Button";

export type PlanCardProps = {
  title: string;
  /** Plain text or rendered markdown. */
  summary?: ReactNode;
  /** One entry per step: plain text or rendered markdown. */
  steps: ReactNode[];
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
    <section aria-label={`Plan: ${title}`} className={cx("flex flex-col overflow-hidden rounded-lg border border-border-strong bg-surface", className)} {...rest}>
      <header className="flex flex-col gap-6 px-20 pt-16 pb-14">
        <div className="flex items-center gap-8">
          <span className="type-label uppercase text-faint">Plan</span>
          <span aria-hidden className="size-3 rounded-pill bg-faint" />
          <span className="type-mono-xs text-faint">
            {steps.length} {steps.length === 1 ? "step" : "steps"}
          </span>
        </div>
        <h3 className="type-ui-strong text-fg">{title}</h3>
        {summary && <div className="type-small text-muted">{summary}</div>}
      </header>
      <ol className="flex flex-col gap-10 border-t border-border px-20 py-14">
        {steps.map((step, i) => (
          <li key={i} className="flex items-baseline gap-12">
            <span aria-hidden className="w-20 shrink-0 text-right type-mono-xs text-faint">
              {i + 1}
            </span>
            <div className="min-w-0 flex-1 type-small text-soft">{step}</div>
          </li>
        ))}
      </ol>
      {onBuild && (
        <footer className="flex items-center gap-12 border-t border-border bg-panel px-20 py-12">
          <Button variant="primary" size="sm" icon="play" loading={building} onClick={onBuild}>
            {building ? "Switching to Build…" : "Build this plan"}
          </Button>
          <span className="type-small text-faint">or reply to change it</span>
        </footer>
      )}
    </section>
  );
}
