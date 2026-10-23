import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Badge } from "../Badge/Badge";

export type DreamChange = { kind: "wrote" | "revised" | "core"; text: ReactNode };

export type DreamJournalEntryProps = {
  /** "Tonight, 03:00 – 03:12". */
  when: ReactNode;
  state?: "complete" | "running" | "failed";
  /** "vector-db-review · #releases · Telegram DM". */
  meta?: ReactNode;
  extraction: ReactNode;
  consolidation: ReactNode;
  changes: DreamChange[];
} & HTMLAttributes<HTMLElement>;

const STATE = {
  complete: { label: "Complete", tone: "neutral" },
  running: { label: "Running", tone: "live" },
  failed: { label: "Failed", tone: "failed" },
} as const;

/** One night's dream (X56): what it read, what it consolidated, and every change it made. */
export function DreamJournalEntry({ when, state = "complete", meta, extraction, consolidation, changes, className, ...rest }: DreamJournalEntryProps) {
  return (
    <article className={cx("flex flex-col gap-14 rounded-xl border border-border bg-surface px-20 py-16", className)} {...rest}>
      <header className="flex flex-wrap items-center gap-10">
        <h3 className="type-heading text-fg">{when}</h3>
        <Badge tone={STATE[state].tone}>{STATE[state].label}</Badge>
        {meta && <span className="type-mono-xs text-faint">{meta}</span>}
      </header>
      <dl className="flex gap-24">
        <div className="flex flex-col gap-2">
          <dt className="type-label uppercase text-faint">Extraction</dt>
          <dd className="m-0 type-ui text-fg">{extraction}</dd>
        </div>
        <div className="flex flex-col gap-2">
          <dt className="type-label uppercase text-faint">Consolidation</dt>
          <dd className="m-0 type-ui text-fg">{consolidation}</dd>
        </div>
      </dl>
      <ul className="flex flex-col gap-10">
        {changes.map((c, i) => (
          <li key={i} className="grid grid-cols-[56px_1fr] items-baseline gap-8">
            <span className={cx("type-mono-xs", c.kind === "core" ? "text-rank-text" : "text-muted")}>{c.kind}</span>
            <span className="type-ui text-fg">{c.text}</span>
          </li>
        ))}
      </ul>
    </article>
  );
}
