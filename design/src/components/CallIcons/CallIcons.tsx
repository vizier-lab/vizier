import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { brands, type BrandName } from "../../icons/brands";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";
import { useDismiss } from "../../lib/dismiss";
import { Floating } from "../Floating/Floating";

/** What a category is drawn with: a design icon, a brand's logo, or the plug with a letter. */
export type CallGlyph = { icon: IconName } | { brand: BrandName } | { letter: string };

export type CallIconItem = {
  /** Stable id of the category ("memory", "mcp:github", "functions"). */
  key: string;
  /** Plain words: "Memory", "GitHub (MCP)". */
  label: string;
  glyph: CallGlyph;
  /** Calls in this category. */
  count: number;
  /** A call in this category failed: its glyph is drawn in the failed colour. */
  failed?: boolean;
  /** Names listed under the category in the popover (the functions it called). */
  details?: { label: string; count: number }[];
};

export type CallIconsProps = {
  /** The categories a run used, most-used first. */
  items: CallIconItem[];
  /** Glyphs shown before `+N`. Default 3. */
  max?: number;
  /** A phone row: one glyph (the failed one if any) and a count of the rest. */
  compact?: boolean;
  /** Calls in the run, for the popover's footer. */
  total?: number;
  /** The run's time, for the popover's footer. */
  duration?: ReactNode;
  className?: string;
};

/** One category's glyph at the trail's size. */
export function CallGlyphIcon({ glyph, size = 14, className }: { glyph: CallGlyph; size?: number; className?: string }) {
  if ("brand" in glyph) {
    const brand = brands[glyph.brand];
    return (
      <svg viewBox="0 0 24 24" width={size - 1} height={size - 1} aria-hidden focusable="false" className={cx("shrink-0 fill-current", className)}>
        <path d={brand.path} />
      </svg>
    );
  }
  if ("letter" in glyph) {
    // An outlined tile with the server's first letter: unknown, but told apart from the others.
    return (
      <span
        aria-hidden
        style={{ width: size, height: size }}
        className={cx("inline-flex shrink-0 items-center justify-center rounded-xs border border-current type-mono-xs-medium leading-none", className)}
      >
        {glyph.letter.slice(0, 1).toUpperCase()}
      </span>
    );
  }
  return <Icon name={glyph.icon} size={size} className={className} />;
}

/** The glyphs a row shows: the first `max`, with a failed category always among them. */
function visibleItems(items: CallIconItem[], max: number): CallIconItem[] {
  const shown = items.slice(0, max);
  const failed = items.find((item) => item.failed);
  if (failed && !shown.includes(failed)) shown[shown.length - 1] = failed;
  return shown;
}

const WIDTH = { wide: "w-84", compact: "w-40" };

/**
 * The categories of tools a python run used, beside its time on the trail: up to three glyphs
 * then `+N`, or one and a count on a phone. The slot is a fixed width, so the times down a trail
 * line up whatever each row used. Hovering it (or a press) lists every category with its count.
 */
export function CallIcons({ items, max = 3, compact = false, total, duration, className }: CallIconsProps) {
  const [open, setOpen] = useState(false);
  const [pinned, setPinned] = useState(false);
  const root = useRef<HTMLSpanElement | null>(null);
  const floating = useRef<HTMLDivElement | null>(null);
  const leave = useRef<ReturnType<typeof setTimeout> | null>(null);
  const id = useId();
  useDismiss(open, () => {
    setOpen(false);
    setPinned(false);
  }, root, floating);
  useEffect(() => () => {
    if (leave.current) clearTimeout(leave.current);
  }, []);

  const width = compact ? WIDTH.compact : WIDTH.wide;
  if (items.length === 0) return <span aria-hidden className={cx("shrink-0", width, className)} />;

  const shown = visibleItems(items, compact ? 1 : max);
  const rest = items.length - shown.length;
  const summary = items.map((item) => `${item.label} ${item.count}`).join(", ");

  // A mouse opens it on hover and closes it on leave, unless a press pinned it open.
  const hoverOpen = (e: React.PointerEvent) => {
    if (e.pointerType !== "mouse") return;
    if (leave.current) clearTimeout(leave.current);
    setOpen(true);
  };
  const hoverClose = (e: React.PointerEvent) => {
    if (e.pointerType !== "mouse" || pinned) return;
    leave.current = setTimeout(() => setOpen(false), 120);
  };

  return (
    <span ref={root} className={cx("relative flex shrink-0 justify-end", width, className)}>
      <button
        type="button"
        aria-label={`Tools used: ${summary}`}
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={() => {
          const next = !(open && pinned);
          setOpen(next);
          setPinned(next);
        }}
        onPointerEnter={hoverOpen}
        onPointerLeave={hoverClose}
        className={cx("flex h-20 items-center rounded-xs text-faint hover:text-muted", compact ? "gap-2" : "gap-6")}
      >
        {shown.map((item) => (
          <CallGlyphIcon key={item.key} glyph={item.glyph} className={item.failed ? "text-failed" : undefined} />
        ))}
        {rest > 0 && <span className="type-mono-xs text-faint">+{rest}</span>}
      </button>
      {open && (
        <Floating anchor={root} ref={floating} align="end" width={232}>
          <div
            id={id}
            role="dialog"
            aria-label="Tools used"
            onPointerEnter={hoverOpen}
            onPointerLeave={hoverClose}
            className="flex flex-col rounded-lg border border-border-strong surface-float p-6 shadow-popover"
          >
            <ul className="flex flex-col">
              {items.map((item) => (
                <li key={item.key} className="flex flex-col">
                  <span className={cx("flex h-26 items-center gap-8 px-6 type-small", item.failed ? "text-failed" : "text-fg")}>
                    <CallGlyphIcon glyph={item.glyph} className={item.failed ? "text-failed" : "text-muted"} />
                    <span className="min-w-0 flex-1 truncate">{item.label}</span>
                    <span className="shrink-0 type-mono-xs text-faint">×{item.count}</span>
                  </span>
                  {item.details?.map((detail) => (
                    <span key={detail.label} className="flex h-22 items-center gap-8 pr-6 pl-24 type-mono-xs text-muted">
                      <span className="min-w-0 flex-1 truncate">{detail.label}</span>
                      <span className="shrink-0 text-faint">×{detail.count}</span>
                    </span>
                  ))}
                </li>
              ))}
            </ul>
            {(total != null || duration) && (
              <div className="mt-4 flex justify-between border-t border-border px-6 pt-6 type-mono-xs text-faint">
                <span>{total != null ? `${total} call${total === 1 ? "" : "s"}` : ""}</span>
                <span>{duration}</span>
              </div>
            )}
          </div>
        </Floating>
      )}
    </span>
  );
}
