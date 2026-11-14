import type { ReactNode } from "react";
import { IconButton } from "../IconButton/IconButton";
import { cx } from "../../lib/cx";

export type StatusPanelProps = {
  /** The panel's title: "Recalled memory", "Background work", "Session files". */
  title: ReactNode;
  /** Right-aligned mono meta: "1,248 passages", "2 running". */
  meta?: ReactNode;
  /** A control between the meta and the x: the language-server switch. */
  headerAction?: ReactNode;
  onClose: () => void;
  /** A search field above the list. */
  search?: ReactNode;
  /** Filter tabs under the search. */
  tabs?: ReactNode;
  /** The list itself; it scrolls. */
  children: ReactNode;
  /** A footer line under the list. */
  footer?: ReactNode;
  /** The panel's width. Unset, it fills the box it floats in. */
  width?: number | string;
  className?: string;
};

/**
 * The panel a status-strip item opens (D62): a title, a search, filters, a scrolling
 * list and a footer, floating above the strip. It closes on its x, on Escape and on a
 * press outside it.
 */
export function StatusPanel({
  title,
  meta,
  headerAction,
  onClose,
  search,
  tabs,
  children,
  footer,
  width,
  className,
}: StatusPanelProps) {
  return (
    <div
      role="dialog"
      aria-label={typeof title === "string" ? title : undefined}
      style={width === undefined ? undefined : { width }}
      className={cx(
        "flex max-h-436 flex-col overflow-hidden rounded-xl border border-border-strong surface-float shadow-popover",
        width === undefined && "w-full",
        className,
      )}
    >
      <div className="flex items-center gap-8 px-16 pt-14 pb-10">
        <span className="min-w-0 flex-1 truncate type-ui-strong text-fg">{title}</span>
        {meta != null && <span className="type-mono-xs text-faint whitespace-nowrap">{meta}</span>}
        {headerAction}
        <IconButton variant="ghost" size="sm" icon="close" aria-label="Close panel" onClick={onClose} />
      </div>
      {search && <div className="px-16 pb-10">{search}</div>}
      {tabs && <div className="px-16 pb-12">{tabs}</div>}
      <div className="min-h-0 flex-1 overflow-y-auto px-16 py-8">{children}</div>
      {footer && (
        <div className="flex items-center justify-between gap-12 border-t border-border px-16 py-10">
          {footer}
        </div>
      )}
    </div>
  );
}