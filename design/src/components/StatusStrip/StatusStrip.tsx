import { useRef, useState, type ReactNode, type RefObject } from "react";
import { Floating } from "../Floating/Floating";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import { useDismiss } from "../../lib/dismiss";
import { cx } from "../../lib/cx";

/** What a strip item counts. */
export type StatusStripItemId = "memory" | "jobs" | "files" | "changes";

export type StatusStripItem = {
  id: StatusStripItemId;
  /** The glyph: memory (book), jobs (layers), files (file), changes (git branch). */
  icon: IconName;
  /** What the count is of, for the accessible name: "recalled passages". */
  label: string;
  /** The count. A section with nothing in it still shows zero. */
  count: number;
  /** Work is running: the item carries a live dot. */
  busy?: boolean;
  /** The item opens something of its own (a dialog) instead of a panel. */
  onSelect?: () => void;
};

export type StatusStripProps = {
  items: StatusStripItem[];
  /** The open item's panel. It is given the close action, so its x closes the panel. */
  children?: (item: StatusStripItem, onClose: () => void) => ReactNode;
  /** The item whose panel is open, when the host owns that state. */
  open?: StatusStripItemId | null;
  onOpenChange?: (id: StatusStripItemId | null) => void;
  /** The panel's width: the composer sets it, so the panel matches the box it floats over. */
  panelWidth?: number | "anchor";
  /** The strip fills its box, so the panel can match that box's width. */
  fullWidth?: boolean;
  /**
   * What the panel hangs from: the composer box, so the panel clears the composer
   * instead of landing on it. Defaults to the strip itself.
   */
  panelAnchor?: RefObject<HTMLElement | null>;
  /**
   * How far above the anchor the panel sits: the composer's height, so the panel
   * clears the composer instead of landing on it.
   */
  panelGap?: number;
  className?: string;
};

/**
 * The chat's status strip (D62): a row of counts under the composer. Each item opens
 * its own panel above the strip, and the open item reads pressed. Every section is
 * always drawn, so a chat that has nothing yet still shows what it does not have.
 * The strip fills the box it is given, and the panel matches that box's width.
 */
export function StatusStrip({
  items,
  children,
  open: controlled,
  onOpenChange,
  panelWidth = "anchor",
  fullWidth = true,
  panelAnchor,
  panelGap,
  className,
}: StatusStripProps) {
  const [own, setOwn] = useState<StatusStripItemId | null>(null);
  const open = controlled ?? own;
  const setOpen = (id: StatusStripItemId | null) => {
    if (controlled === undefined) setOwn(id);
    onOpenChange?.(id);
  };
  const anchor = useRef<HTMLDivElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  useDismiss(open !== null, () => setOpen(null), panel, anchor);

  if (items.length === 0) return null;
  const active = items.find((i) => i.id === open) ?? null;

  return (
    <div ref={anchor} className={cx("flex items-center gap-4", fullWidth && "w-full", className)}>
      {items.map((item) => (
        <button
          key={item.id}
          type="button"
          aria-expanded={item.onSelect ? undefined : open === item.id}
          aria-label={`${item.count} ${item.label}`}
          onClick={() => (item.onSelect ? item.onSelect() : setOpen(open === item.id ? null : item.id))}
          className={cx(
            "inline-flex h-29 items-center gap-4 rounded-pill px-8 transition-colors",
            open === item.id
              ? "bg-raised text-fg"
              : "text-muted enabled:hover:bg-raised enabled:hover:text-fg",
          )}
        >
          <Icon name={item.icon} size={15} />
          <span className="type-mono-xs-medium">{item.count}</span>
          {item.busy && <span aria-hidden className="size-6 rounded-pill bg-live" />}
        </button>
      ))}
      {active && children && (
        <Floating
          anchor={panelAnchor ?? anchor}
          align="start"
          width={panelWidth}
          matchWidth={panelWidth === "anchor"}
          gap={panelGap}
        >
          <div ref={panel}>{children(active, () => setOpen(null))}</div>
        </Floating>
      )}
    </div>
  );
}