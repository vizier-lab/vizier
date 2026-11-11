import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";
import { IconButton } from "../IconButton/IconButton";

export type SectionHeadingProps = {
  /** Uppercase mono label, e.g. "Agents". */
  label: string;
  /** A + beside the label, e.g. New agent. Omit it when the person may not add. */
  onAdd?: () => void;
  /** The + button's name, e.g. "New agent". */
  addLabel?: string;
  /** Heading level for the label. */
  level?: 2 | 3;
} & HTMLAttributes<HTMLDivElement>;

/** A list's heading on a phone (spec 056): the label, and the + that adds to the list on its side. */
export function SectionHeading({ label, onAdd, addLabel, level = 2, className, ...rest }: SectionHeadingProps) {
  const H = `h${level}` as const;
  return (
    <div className={cx("flex min-h-36 items-center gap-8", className)} {...rest}>
      <H className="min-w-0 flex-1 truncate type-label uppercase text-faint">{label}</H>
      {onAdd && (
        <IconButton variant="ghost" icon="plus" aria-label={addLabel ?? `Add to ${label.toLowerCase()}`} onClick={onAdd} className="bg-surface" />
      )}
    </div>
  );
}
