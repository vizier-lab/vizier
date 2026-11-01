import type { ButtonHTMLAttributes } from "react";
import { cx } from "../../lib/cx";

export type CheckpointDividerProps = {
  /** "handed over at 81% of 200k"; omit for a reset that carried nothing over. */
  detail?: string;
  /** What ended the context: a checkpoint (the default) or `/lobotomy`, which keeps none of it. */
  kind?: "checkpoint" | "lobotomy";
  /** Whether the handover summary below is open. */
  open?: boolean;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** Marks where the context was reset (D48). With a handover it opens the summary. */
export function CheckpointDivider({ detail, kind = "checkpoint", open, className, type = "button", ...rest }: CheckpointDividerProps) {
  const label = detail
    ? `Checkpoint · ${detail}`
    : kind === "lobotomy"
      ? "Lobotomy · nothing carried over"
      : "Checkpoint · no handover";
  const line = <span aria-hidden className="h-px flex-1 bg-border-strong" />;
  return (
    <div role="separator" aria-label={label} className={cx("flex items-center gap-12", className)}>
      {line}
      {detail ? (
        <button type={type} aria-expanded={open ?? false} className="rounded-xs type-label uppercase text-faint hover:text-fg" {...rest}>
          {label}
        </button>
      ) : (
        <span className="type-label uppercase text-faint">{label}</span>
      )}
      {line}
    </div>
  );
}
