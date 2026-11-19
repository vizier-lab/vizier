import { useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { motion, useReducedMotion } from "motion/react";
import { Icon } from "../../icons/Icon";
import { StatusStripButton, type StatusStripItem, type StatusStripItemId } from "../StatusStrip/StatusStrip";
import { StatusPanelInDeck } from "../StatusStrip/StatusPanel";
import { useDismiss } from "../../lib/dismiss";
import { cx } from "../../lib/cx";
import { motionDurations, motionEases } from "../../tokens/tokens";

/** The deck's lip: the back panel's height above the composer when it is closed. */
const LIP = 30;

/** Something that takes the deck's pressed slot without being a strip item: a `/btw` aside. */
export type ComposerDeckAside = {
  /** Its name in the header: "/btw". */
  label: ReactNode;
  /** A short note after the name: "never saved". */
  note?: ReactNode;
  /** What the sheet shows. */
  content: ReactNode;
  onClose: () => void;
};

export type ComposerDeckProps = {
  /** The strip's counts. Every section is always drawn, at zero too. */
  items: StatusStripItem[];
  /** The item whose sheet is open. */
  open: StatusStripItemId | null;
  onOpenChange: (id: StatusStripItemId | null) => void;
  /** The open item's sheet, given the close action. */
  sheet?: (item: StatusStripItem, onClose: () => void) => ReactNode;
  /** An aside that takes the pressed slot and the sheet, e.g. `/btw`. Wins over `open`. */
  aside?: ComposerDeckAside | null;
  /** At the right end of the header: the model picker, or a host's own status such as a branch. */
  trailing?: ReactNode;
  /** The composer: the front panel. */
  children: ReactNode;
  className?: string;
};

/**
 * The composer and its back panel (D62, drawn on "Composer · two panels"). The back panel is
 * an inset card of frosted glass, in the deck's colour, standing on the composer's top edge; an
 * open sheet is a solid card in the composer's own colour, inset so a glass rim frames it. The composer, solid,
 * casts its shadow up onto it. Closed, only its lip shows, with the status strip; an item
 * opens its sheet by growing the card upward over the conversation, and the strip becomes the sheet's header (the open
 * item pressed, the x at the right). The composer never moves.
 * The sheet closes on its x, on Escape and on a press outside the deck.
 *
 * Motion: the sheet grows to its measured height on the arrive ease and folds back faster on
 * the standard ease, so the card rises out of the lip and settles into it again; switching
 * sections glides between the two heights while the new section fades in. With reduced
 * motion the height snaps and only the fades remain.
 */
export function ComposerDeck({ items, open, onOpenChange, sheet, aside, trailing, children, className }: ComposerDeckProps) {
  const deck = useRef<HTMLDivElement>(null);
  const indicator = `deck-tab-${useId()}`;
  const active = aside ? null : (items.find((i) => i.id === open) ?? null);
  const expanded = !!aside || !!active;
  const close = () => (aside ? aside.onClose() : onOpenChange(null));
  const reduced = useReducedMotion();

  // The sheet's content, kept through the closing fold so it does not vanish mid-way.
  const content = aside ? aside.content : active ? sheet?.(active, close) : null;
  const contentKey = aside ? "aside" : (active?.id ?? null);
  const kept = useRef<{ key: string; node: ReactNode } | null>(null);
  if (expanded && contentKey) kept.current = { key: contentKey, node: content };
  const shown = expanded ? { key: contentKey!, node: content } : kept.current;

  // The sheet's natural height, followed as its content changes (a search narrowing a list).
  const inner = useRef<HTMLDivElement>(null);
  const [height, setHeight] = useState(0);
  useLayoutEffect(() => {
    const el = inner.current;
    if (!el) return;
    setHeight(el.offsetHeight);
    const ro = new ResizeObserver(() => setHeight(el.offsetHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, [!!shown]);
  useDismiss(!!active, () => onOpenChange(null), deck);

  const press = (item: StatusStripItem) => {
    if (item.onSelect) return item.onSelect();
    aside?.onClose();
    onOpenChange(open === item.id && !aside ? null : item.id);
  };

  return (
    <div className={cx("relative", className)} style={{ paddingTop: LIP }}>
      <div
        ref={deck}
        // Anchored at the composer's top edge, so opening grows it upward only.
        style={{ bottom: `calc(100% - ${LIP}px)` }}
        className={cx(
          "absolute inset-x-24 z-0 flex flex-col overflow-hidden rounded-t-xl border border-b-0 bg-deck/60 backdrop-blur-lg backdrop-saturate-150 transition-[border-color,box-shadow] duration-(--vz-duration-base) max-lg:inset-x-12",
          // A translucent edge, so the frosted rim reads as glass rather than a drawn line.
          expanded ? "border-border-strong/60 shadow-popover" : "border-border/60",
        )}
      >
        <div
          className={cx(
            // 6px, the rim's width: the first pill and the x line up with the card's edges below.
            "flex shrink-0 items-center gap-4 px-6",
          )}
          style={{ height: LIP }}
        >
          {aside && (
            <span className="inline-flex h-24 shrink-0 items-center gap-6 rounded-pill border border-border-strong bg-surface px-10">
              <span className="type-code font-medium text-live-text">{aside.label}</span>
              {aside.note && <span className="type-small text-muted whitespace-nowrap">{aside.note}</span>}
            </span>
          )}
          <div className="flex min-w-0 items-center gap-4 overflow-x-auto [scrollbar-width:none]">
            {items.map((item) => (
              <StatusStripButton
                key={item.id}
                item={item}
                pressed={active?.id === item.id}
                onPress={() => press(item)}
                onRaised
                indicator={indicator}
              />
            ))}
          </div>
          <span className="flex-1" />
          {trailing && <div className="flex min-w-0 max-w-[55%] shrink items-center justify-end">{trailing}</div>}
          {expanded && (
            <button
              type="button"
              aria-label={aside ? "Close side chat" : "Close panel"}
              onClick={close}
              // Full strength, like the strip: the header is glass, and a muted glyph is lost on it.
              className="flex size-24 shrink-0 items-center justify-center rounded-sm text-fg hover:bg-surface"
            >
              <Icon name="close" size={13} />
            </button>
          )}
        </div>
        <motion.div
          initial={false}
          aria-hidden={!expanded || undefined}
          inert={!expanded}
          animate={{ height: expanded ? height : 0 }}
          transition={
            reduced
              ? { duration: 0 }
              : expanded
                ? { duration: motionDurations.slow, ease: motionEases.arrive }
                : { duration: motionDurations.base, ease: motionEases.standard }
          }
          onAnimationComplete={() => {
            if (!expanded) kept.current = null;
          }}
          // The sheet is a solid card in the composer's colour, inset in the deck's glass so a
          // frosted rim runs down both sides.
          className="mx-6 shrink-0 overflow-hidden rounded-t-lg"
        >
          {shown && (
            <StatusPanelInDeck.Provider value={true}>
              {/* The card's colour and edge sit on this layer, which never fades, so a section
                  fading in never shows the glass through it. */}
              <div ref={inner} className="rounded-t-lg border border-b-0 border-border bg-surface">
                <motion.div
                  key={shown.key}
                  initial={{ opacity: 0 }}
                  animate={{ opacity: expanded ? 1 : 0 }}
                  transition={{ duration: expanded ? motionDurations.base : motionDurations.fast, ease: motionEases.standard }}
                  className="flex flex-col"
                >
                  {shown.node}
                </motion.div>
              </div>
            </StatusPanelInDeck.Provider>
          )}
        </motion.div>
      </div>
      <div className="relative z-10">{children}</div>
    </div>
  );
}
