import { createContext, useContext, type ReactNode } from "react";
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
 * Inside the composer deck, a panel is the deck's sheet: the deck draws the card, names the
 * open section and closes it, so the panel drops its own border, title and x.
 */
export const StatusPanelInDeck = createContext(false);

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
  const inDeck = useContext(StatusPanelInDeck);
  if (inDeck) {
    return (
      <div
        role="region"
        aria-label={typeof title === "string" ? title : undefined}
        className={cx("flex max-h-[min(436px,55vh)] flex-col", className)}
      >
        {(meta != null || headerAction) && (
          <div className="flex items-center justify-end gap-8 px-16 pt-10">
            {meta != null && <span className="type-mono-xs text-faint whitespace-nowrap">{meta}</span>}
            {headerAction}
          </div>
        )}
        {search && <div className="px-16 pt-10">{search}</div>}
        {tabs && <div className="px-16 pt-10">{tabs}</div>}
        <div className="min-h-0 flex-1 overflow-y-auto px-16 py-8">{children}</div>
        {footer && (
          <div className="flex items-center justify-between gap-12 border-t border-border px-16 py-10">{footer}</div>
        )}
      </div>
    );
  }
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