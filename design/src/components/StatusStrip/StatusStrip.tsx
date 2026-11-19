import { useRef, useState, type ReactNode, type RefObject } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Floating } from "../Floating/Floating";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import { useDismiss } from "../../lib/dismiss";
import { cx } from "../../lib/cx";
import { motionDurations, motionEases } from "../../tokens/tokens";

/** What a strip item counts. */
export type StatusStripItemId = "memory" | "work" | "subagents" | "files" | "changes" | "lsp";

/** The dot beside a count: running (steady), starting (pulsing), needs attention, or failed. */
export type StatusStripDot = "live" | "pulse" | "warn" | "fail";

export type StatusStripItem = {
  id: StatusStripItemId;
  /** The glyph: memory (book), work (layers), subagents (users), files (file), changes (git branch), lsp (code). */
  icon: IconName;
  /** What the count is of, for the accessible name: "recalled passages". */
  label: string;
  /** The count. A section with nothing in it still shows zero. */
  count: number;
  /** Work is running: the item carries a live dot. Same as `dot: "live"`. */
  busy?: boolean;
  /** The item's dot. Wins over `busy`. */
  dot?: StatusStripDot;
  /** Read instead of the count: `off` for a section that is switched off. */
  display?: string;
  /** The item opens something of its own (a dialog) instead of a panel. */
  onSelect?: () => void;
};

const DOT_CLASS: Record<StatusStripDot, string> = {
  live: "bg-live",
  pulse: "bg-live animate-shimmer",
  warn: "bg-retrying",
  fail: "bg-failed",
};

function dotOf(item: StatusStripItem): StatusStripDot | null {
  return item.dot ?? (item.busy ? "live" : null);
}

/**
 * One strip item: its glyph, its count and its dot. Pressed while its panel is open; with
 * `label`, the pressed item also names its section (the composer deck's header).
 */
export function StatusStripButton({
  item,
  pressed,
  label,
  onPress,
  onRaised,
  indicator,
}: {
  item: StatusStripItem;
  pressed: boolean;
  /** Shown after the count, e.g. "Background work" while that sheet is open. */
  label?: string;
  onPress: () => void;
  /**
   * Drawn in the composer deck's glass header: every item at full strength so it reads over
   * whatever shows through, hover on the surface colour, and the pressed item in the live colour.
   */
  onRaised?: boolean;
  /**
   * A layout id shared by one strip's buttons: the pressed pill becomes one indicator that
   * slides from item to item instead of each button lighting up on its own.
   */
  indicator?: string;
}) {
  const sliding = !!indicator;
  const dot = dotOf(item);
  return (
    <button
      type="button"
      aria-expanded={item.onSelect ? undefined : pressed}
      aria-label={item.display ? `${item.display}: ${item.label}` : `${item.count} ${item.label}`}
      onClick={onPress}
      className={cx(
        // isolate keeps the sliding pill behind the content without letting it fall behind the strip.
        "relative isolate inline-flex shrink-0 items-center gap-4 rounded-pill px-8 transition-colors",
        onRaised ? "h-24" : "h-29",
        onRaised
          ? pressed
            ? cx("text-on-live", !sliding && "bg-live")
            : "text-fg enabled:hover:bg-surface"
          : pressed
            ? "bg-raised text-fg"
            : "text-muted enabled:hover:bg-raised enabled:hover:text-fg",
      )}
    >
      {sliding && pressed && (
        <motion.span
          aria-hidden
          layoutId={indicator}
          className={cx("absolute inset-0 -z-10 rounded-pill", onRaised ? "bg-live" : "bg-surface ring-1 ring-inset ring-border-strong")}
          transition={{ type: "spring", stiffness: 520, damping: 34, mass: 0.9 }}
        />
      )}
      <Icon name={item.icon} size={15} />
      <span className="type-mono-xs-medium">{item.display ?? item.count}</span>
      {dot && (
        <span
          aria-hidden
          className={cx(
            "size-6 rounded-pill",
            // In the deck's live pill a live dot would vanish: it takes the pill's text colour.
            onRaised && pressed && (dot === "live" || dot === "pulse")
              ? cx("bg-on-live", dot === "pulse" && "animate-shimmer")
              : DOT_CLASS[dot],
          )}
        />
      )}
      {sliding ? (
        <AnimatePresence initial={false}>
          {label && (
            <motion.span
              key="label"
              initial={{ width: 0, opacity: 0 }}
              animate={{ width: "auto", opacity: 1, transition: { duration: motionDurations.base, ease: motionEases.arrive } }}
              exit={{ width: 0, opacity: 0, transition: { duration: motionDurations.fast, ease: motionEases.standard } }}
              className="overflow-hidden type-small-medium whitespace-nowrap"
            >
              <span className="pl-2">{label}</span>
            </motion.span>
          )}
        </AnimatePresence>
      ) : (
        label && <span className="ml-2 animate-fade-in type-small-medium whitespace-nowrap">{label}</span>
      )}
    </button>
  );
}

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
        <StatusStripButton
          key={item.id}
          item={item}
          pressed={open === item.id}
          onPress={() => (item.onSelect ? item.onSelect() : setOpen(open === item.id ? null : item.id))}
        />
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