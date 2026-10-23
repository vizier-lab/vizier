import { cloneElement, useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type ReactElement } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { DEFAULT_RECENT, EMOJI, searchEmoji } from "./emoji";

export type EmojiPickerProps = {
  onSelect: (emoji: string) => void;
  /** Most recent first. */
  recent?: string[];
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  /** The button that opens it (AddReaction). */
  trigger: ReactElement<{ onClick?: () => void; "aria-expanded"?: boolean; "aria-haspopup"?: "dialog" }>;
  className?: string;
};

const COLUMNS = 8;

/**
 * A searchable emoji grid in a popover (X78), for reactions. Arrow keys move
 * across the grid, Enter picks, Esc closes. The emoji list is bundled.
 */
export function EmojiPicker({ onSelect, recent = DEFAULT_RECENT, open: openProp, onOpenChange, trigger, className }: EmojiPickerProps) {
  const id = useId();
  const [openState, setOpenState] = useState(false);
  const open = openProp ?? openState;
  const setOpen = (v: boolean) => {
    setOpenState(v);
    onOpenChange?.(v);
  };
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const root = useRef<HTMLDivElement>(null);

  const shown = useMemo(() => searchEmoji(query), [query]);
  const recents = useMemo(() => (query ? [] : recent.filter((e) => EMOJI.some((x) => x.emoji === e))), [query, recent]);
  const cells = useMemo(() => [...recents.map((e) => e), ...shown.map((e) => e.emoji)], [recents, shown]);

  useEffect(() => setActive(0), [query]);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", onDown);
    return () => document.removeEventListener("pointerdown", onDown);
  });

  const pick = (emoji: string) => {
    onSelect(emoji);
    setOpen(false);
    setQuery("");
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    const move = (d: number) => {
      e.preventDefault();
      setActive((a) => Math.max(0, Math.min(cells.length - 1, a + d)));
    };
    if (e.key === "ArrowRight") move(1);
    else if (e.key === "ArrowLeft") move(-1);
    else if (e.key === "ArrowDown") move(COLUMNS);
    else if (e.key === "ArrowUp") move(-COLUMNS);
    else if (e.key === "Enter" && cells[active]) {
      e.preventDefault();
      pick(cells[active]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      setOpen(false);
    }
  };

  const cell = (emoji: string, index: number, name?: string) => (
    <button
      key={`${index}-${emoji}`}
      id={`${id}-${index}`}
      type="button"
      role="option"
      aria-selected={index === active}
      aria-label={name ?? emoji}
      tabIndex={-1}
      onPointerMove={() => setActive(index)}
      onClick={() => pick(emoji)}
      className={cx("flex size-32 items-center justify-center rounded-sm type-body", index === active && "bg-raised")}
    >
      {emoji}
    </button>
  );

  return (
    <div ref={root} className="relative inline-flex">
      {cloneElement(trigger, { onClick: () => setOpen(!open), "aria-expanded": open, "aria-haspopup": "dialog" })}
      {open && (
        <div
          role="dialog"
          aria-label="Add reaction"
          className={cx("absolute top-full left-0 z-30 mt-6 flex w-304 flex-col overflow-hidden rounded-xl border border-border-strong bg-surface shadow-popover", className)}
        >
          <label className="flex items-center gap-8 border-b border-border px-12 py-10">
            <Icon name="search" size={14} tone="faint" />
            <input
              autoFocus
              role="combobox"
              aria-expanded
              aria-controls={`${id}-grid`}
              aria-activedescendant={cells.length ? `${id}-${active}` : undefined}
              aria-label="Search emoji"
              placeholder="Search emoji"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={onKeyDown}
              className="min-w-0 flex-1 bg-transparent type-body text-fg caret-live outline-none placeholder:text-faint"
            />
          </label>
          <div id={`${id}-grid`} role="listbox" aria-label="Emoji" className="max-h-240 overflow-y-auto p-8">
            {recents.length > 0 && (
              <>
                <p className="px-4 pb-4 type-label uppercase text-faint">Recent</p>
                <div className="grid grid-cols-8">{recents.map((e, i) => cell(e, i))}</div>
                <p className="px-4 pt-8 pb-4 type-label uppercase text-faint">All</p>
              </>
            )}
            {shown.length === 0 ? (
              <p className="px-4 py-12 type-small text-faint">Nothing matched</p>
            ) : (
              <div className="grid grid-cols-8">{shown.map((e, i) => cell(e.emoji, recents.length + i, e.name))}</div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
