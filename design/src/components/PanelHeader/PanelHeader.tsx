import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { IconButton } from "../IconButton/IconButton";

export type PanelHeaderProps = {
  title: ReactNode;
  /** Right-aligned mono meta: "4 passages", "3 running". */
  meta?: ReactNode;
  /** A right-aligned action (a TextLink or small Button). */
  action?: ReactNode;
  /** Shows a close ×, for side panels. */
  onClose?: () => void;
  /** Heading level for the title. */
  level?: 2 | 3 | 4;
} & HTMLAttributes<HTMLDivElement>;

/** A panel's title with its meta on the right (X2). */
export function PanelHeader({ title, meta, action, onClose, level = 3, className, ...rest }: PanelHeaderProps) {
  const H = `h${level}` as const;
  return (
    <div className={cx("flex min-h-16 items-center gap-8", className)} {...rest}>
      <H className="min-w-0 flex-1 truncate type-ui-strong text-fg">{title}</H>
      {meta != null && <span className="type-mono-xs text-faint whitespace-nowrap">{meta}</span>}
      {action}
      {onClose && <IconButton variant="ghost" size="sm" icon="close" aria-label="Close panel" onClick={onClose} />}
    </div>
  );
}
