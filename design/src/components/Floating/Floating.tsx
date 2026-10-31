import { forwardRef, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";

export type FloatingProps = {
  /** The element the popup hangs from (the trigger's wrapper). */
  anchor: RefObject<HTMLElement | null>;
  /** Which edge of the anchor the popup lines up with. */
  align?: "start" | "end";
  /** Match the anchor's width at least (a Select's list), or size to its content. */
  matchWidth?: boolean;
  /** A fixed width, or `anchor` for exactly the trigger's width (a menu the size of its card). */
  width?: number | "anchor";
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
};

const GAP = 6;
const MARGIN = 8;

/**
 * A popup that escapes clipping: it is drawn with fixed coordinates from the anchor, inside the
 * anchor's own <dialog> when there is one (a modal's top layer would hide it in the body), and
 * flips above the anchor when there is no room below. Tables, cards and scroll areas that clip
 * their children (overflow) no longer cut a Select or Picker off.
 */
export const Floating = forwardRef<HTMLDivElement, FloatingProps>(function Floating(
  { anchor, align = "start", matchWidth, width, children, className, style },
  forwarded,
) {
  const own = useRef<HTMLDivElement | null>(null);
  const [pos, setPos] = useState<CSSProperties>({ position: "fixed", visibility: "hidden", top: 0, left: 0 });
  const host = anchor.current?.closest("dialog") ?? document.body;

  useLayoutEffect(() => {
    const place = () => {
      const a = anchor.current;
      const p = own.current;
      if (!a || !p) return;
      const r = a.getBoundingClientRect();
      const h = p.offsetHeight;
      const w = p.offsetWidth;
      const below = window.innerHeight - r.bottom - MARGIN;
      const above = r.top - MARGIN;
      const up = h + GAP > below && above > below;
      const top = up ? Math.max(MARGIN, r.top - GAP - h) : r.bottom + GAP;
      let left = align === "end" ? r.right - w : r.left;
      left = Math.max(MARGIN, Math.min(left, window.innerWidth - w - MARGIN));
      setPos({
        position: "fixed",
        top,
        left,
        width: width === "anchor" ? r.width : width,
        minWidth: matchWidth ? r.width : undefined,
        maxHeight: Math.max(160, (up ? above : below) - GAP),
        visibility: "visible",
      });
    };
    place();
    // A scroll in any ancestor moves the anchor: follow it.
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    const ro = new ResizeObserver(place);
    if (own.current) ro.observe(own.current);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
      ro.disconnect();
    };
  }, [anchor, align, matchWidth, width]);

  return createPortal(
    <div
      data-floating
      ref={(el) => {
        own.current = el;
        if (typeof forwarded === "function") forwarded(el);
        else if (forwarded) forwarded.current = el;
      }}
      className={className}
      style={{ ...pos, ...style, zIndex: 60 }}
    >
      {children}
    </div>,
    host,
  );
});

/** True for a pointer target inside any Floating popup. */
export const inFloating = (target: EventTarget | null) => target instanceof Element && !!target.closest("[data-floating]");
