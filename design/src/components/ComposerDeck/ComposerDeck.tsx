import { useRef, type ReactNode } from "react";
import { IconButton } from "../IconButton/IconButton";
import { StatusStripButton, type StatusStripItem, type StatusStripItemId } from "../StatusStrip/StatusStrip";
import { StatusPanelInDeck } from "../StatusStrip/StatusPanel";
import { useDismiss } from "../../lib/dismiss";
import { cx } from "../../lib/cx";

/** The deck's lip: the back panel's height above the composer when it is closed. */
const LIP = 36;

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
 * an inset card of frosted glass standing on the composer's top edge; the composer, frosted
 * too, casts its shadow up onto it. Nothing runs under the composer, so its glass stays clear. Closed, only its lip shows, with the status strip;
 * an item opens its sheet by growing the card upward over the conversation, and the strip
 * becomes the sheet's header (the open item pressed and named, the x at the right). The
 * composer never moves. The sheet closes on its x, on Escape and on a press outside the deck.
 */
export function ComposerDeck({ items, open, onOpenChange, sheet, aside, trailing, children, className }: ComposerDeckProps) {
  const deck = useRef<HTMLDivElement>(null);
  const active = aside ? null : (items.find((i) => i.id === open) ?? null);
  const expanded = !!aside || !!active;
  const close = () => (aside ? aside.onClose() : onOpenChange(null));
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
          "absolute inset-x-24 z-0 flex flex-col overflow-hidden rounded-t-xl border border-b-0 bg-raised/60 backdrop-blur-lg backdrop-saturate-150 max-lg:inset-x-12",
          expanded ? "border-border-strong shadow-popover" : "border-border",
        )}
      >
        <div className={cx("flex shrink-0 items-center gap-4 px-6", expanded && "border-b border-border")} style={{ height: LIP }}>
          {aside && (
            <span className="inline-flex h-29 shrink-0 items-center gap-6 rounded-pill border border-border-strong bg-surface px-10">
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
                label={active?.id === item.id ? sectionName(item) : undefined}
                onPress={() => press(item)}
                onRaised
              />
            ))}
          </div>
          <span className="flex-1" />
          {trailing && <div className="flex min-w-0 max-w-[55%] shrink items-center justify-end">{trailing}</div>}
          {expanded && (
            <IconButton variant="ghost" size="sm" icon="close" aria-label={aside ? "Close side chat" : "Close panel"} onClick={close} />
          )}
        </div>
        {expanded && (
          <StatusPanelInDeck.Provider value={true}>
            <div className="flex min-h-0 flex-col">{aside ? aside.content : active && sheet?.(active, close)}</div>
          </StatusPanelInDeck.Provider>
        )}
      </div>
      <div className="relative z-10">{children}</div>
    </div>
  );
}

const SECTION_NAMES: Partial<Record<StatusStripItemId, string>> = {
  memory: "Recalled memory",
  work: "Background work",
  subagents: "Subagents",
  files: "Session files",
  changes: "Changes",
  lsp: "Language servers",
};

function sectionName(item: StatusStripItem) {
  return SECTION_NAMES[item.id] ?? item.label;
}
