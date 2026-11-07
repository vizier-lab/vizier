import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Icon } from "../../icons/Icon";

export type MentionNoteProps = {
  children?: ReactNode;
} & HTMLAttributes<HTMLDivElement>;

/**
 * Why a mention did not wake anyone (X61): "scribe is restarting, so it was not woken". Failed tone
 * only. A successful mention draws nothing beyond its chip, so there is no other variant.
 */
export function MentionNote({ className, children, ...rest }: MentionNoteProps) {
  return (
    <div role="status" className={cx("flex items-center gap-6 type-small text-failed-text", className)} {...rest}>
      <Icon name="alert" size={13} aria-hidden />
      <span>{children}</span>
    </div>
  );
}
