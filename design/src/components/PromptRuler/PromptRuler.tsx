import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent, type WheelEvent } from "react";
import { cx } from "../../lib/cx";
import { STEP, clamp, count, indexAt, marksAround, wheelSteps } from "./ruler";

export type PromptPreview = {
  /** The start of what the person said. */
  text: string;
  /** When they said it, already written for reading ("21:24", "14 Nov 21:24"). */
  time?: string;
};

export type PromptRulerProps = {
  /** How many prompts the conversation has, loaded or not. */
  total: number;
  /** 0-based index of the prompt being read: the long mark. */
  current: number;
  /** What the label says for a prompt; undefined while it is not known yet. */
  preview?: (index: number) => PromptPreview | undefined;
  /** Go to a prompt: a click on its mark, or Enter on the one chosen with the arrow keys. */
  onJump: (index: number) => void;
  /** Marks shown on each side of the middle; a longer conversation is windowed. */
  radius?: number;
  className?: string;
  style?: CSSProperties;
};

/** How long after the pointer leaves a browsed ruler it goes back to the prompt being read. */
const RETURN_MS = 900;
/** Movement below this is a click on a mark, not a drag. */
const DRAG_PX = 3;

/**
 * A ruler of a conversation's prompts (X92): one mark per prompt, evenly spaced (never a
 * scroll bar), the prompt being read long. Hovering names a prompt and clicking jumps to it.
 * The wheel over it, a drag or the arrow keys browse the ruler without moving the
 * conversation; a click (or Enter) is what jumps. A long conversation shows a window of the
 * prompts, always full: near either end it stops at the first or last prompt. Every tenth is
 * numbered, with a counter below.
 */
export function PromptRuler({ total, current, preview, onJump, radius = 10, className, style }: PromptRulerProps) {
  // The prompt the window is centred on: the one being read, or where browsing has got to.
  const [center, setCenter] = useState(current);
  const [browsing, setBrowsing] = useState<"pointer" | "keys" | null>(null);
  // The pointer's height from the ruler's middle, while it is over the ruler.
  const [hoverY, setHoverY] = useState<number | null>(null);
  const centerRef = useRef(center);
  centerRef.current = center;
  const wheelCarry = useRef(0);
  const drag = useRef<{ y: number; from: number; moved: boolean } | null>(null);
  const leaving = useRef<ReturnType<typeof setTimeout> | null>(null);
  const root = useRef<HTMLDivElement>(null);

  // Follow the conversation, except while the person is browsing the ruler.
  useEffect(() => {
    if (!browsing) setCenter(clamp(current, 0, Math.max(0, total - 1)));
  }, [current, total, browsing]);
  useEffect(() => () => {
    if (leaving.current) clearTimeout(leaving.current);
  }, []);

  if (total <= 0) return null;
  const { marks, windowed } = marksAround(total, center, current, radius);
  const height = (radius * 2 + 1) * STEP;
  const middle = Math.floor(height / 2);
  const hovered = hoverY != null ? indexAt(marks, hoverY) : null;
  const shown = hovered ?? (browsing === "keys" ? center : null);
  const shownMark = shown != null ? marks.find((m) => m.index === shown) : undefined;
  const label = shown != null ? preview?.(shown) : undefined;

  const browseTo = (index: number, how: "pointer" | "keys") => {
    const next = clamp(index, 0, total - 1);
    // Set at once, not on the next render: a burst of wheel events each builds on the last.
    centerRef.current = next;
    setCenter(next);
    setBrowsing(how);
  };
  const stopBrowsing = () => {
    setBrowsing(null);
    wheelCarry.current = 0;
  };
  const jump = (index: number) => {
    stopBrowsing();
    onJump(index);
  };

  const pointerY = (e: PointerEvent) => e.clientY - (root.current?.getBoundingClientRect().top ?? 0) - middle;

  const onWheel = (e: WheelEvent) => {
    const { steps, carry } = wheelSteps(wheelCarry.current, e.deltaY, e.deltaMode);
    wheelCarry.current = carry;
    if (steps !== 0) browseTo(centerRef.current + steps, "pointer");
  };

  const onPointerEnter = () => {
    if (leaving.current) clearTimeout(leaving.current);
    leaving.current = null;
  };
  const onPointerDown = (e: PointerEvent) => {
    if (e.button !== 0) return;
    // Captured only once it turns into a drag: capturing on press would send the click to the
    // ruler instead of the mark under the pointer.
    drag.current = { y: e.clientY, from: centerRef.current, moved: false };
  };
  const onPointerMove = (e: PointerEvent) => {
    setHoverY(pointerY(e));
    const d = drag.current;
    if (!d) return;
    const dy = e.clientY - d.y;
    if (!d.moved && Math.abs(dy) < DRAG_PX) return;
    if (!d.moved) (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    d.moved = true;
    // Dragging the ruler down brings earlier prompts into view, as dragging a page does.
    const next = clamp(d.from - Math.round(dy / STEP), 0, total - 1);
    if (next !== centerRef.current) browseTo(next, "pointer");
  };
  const onPointerUp = (e: PointerEvent) => {
    drag.current = null;
    (e.currentTarget as HTMLElement).releasePointerCapture?.(e.pointerId);
  };
  const onPointerLeave = () => {
    setHoverY(null);
    if (browsing !== "pointer") return;
    if (leaving.current) clearTimeout(leaving.current);
    leaving.current = setTimeout(() => {
      leaving.current = null;
      stopBrowsing();
    }, RETURN_MS);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const from = browsing ? centerRef.current : current;
    const step = { ArrowUp: -1, ArrowDown: 1, PageUp: -10, PageDown: 10 }[e.key];
    if (step != null) browseTo(from + step, "keys");
    else if (e.key === "Home") browseTo(0, "keys");
    else if (e.key === "End") browseTo(total - 1, "keys");
    else if (e.key === "Enter" || e.key === " ") jump(browsing ? centerRef.current : current);
    else if (e.key === "Escape" && browsing) stopBrowsing();
    else return;
    e.preventDefault();
  };

  const chosen = browsing === "keys" ? center : current;
  const chosenPreview = preview?.(chosen);
  return (
    <div
      ref={root}
      role="slider"
      tabIndex={0}
      aria-label="Prompts"
      aria-orientation="vertical"
      aria-valuemin={1}
      aria-valuemax={total}
      aria-valuenow={chosen + 1}
      aria-valuetext={chosenPreview ? `${chosen + 1} of ${total}: ${chosenPreview.text}` : `${chosen + 1} of ${total}`}
      className={cx("relative w-48 touch-none select-none rounded-xs outline-none focus-visible:outline-2 focus-visible:outline-focus", className)}
      style={{ height: height + (windowed ? 24 : 0), ...style }}
      onWheel={onWheel}
      onPointerEnter={onPointerEnter}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onPointerLeave={onPointerLeave}
      onKeyDown={onKeyDown}
      onBlur={() => browsing === "keys" && stopBrowsing()}
    >
      {marks.map((mark) => {
        const lit = mark.index === shown;
        const width = mark.kind === "current" ? 16 : lit ? 12 : mark.kind === "major" ? 12 : 8;
        return (
          <button
            key={mark.index}
            type="button"
            tabIndex={-1}
            aria-hidden
            className="absolute left-0 flex h-14 w-full cursor-pointer items-center outline-none transition-[top] duration-[var(--vz-duration-fast)] motion-reduce:transition-none"
            style={{ top: middle + mark.offset * STEP - STEP / 2 }}
            // A mark is pressed with the pointer only; the ruler itself keeps keyboard focus.
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => jump(mark.index)}
          >
            <span
              className={cx(
                "block h-2 rounded-pill transition-[width,background-color,opacity] duration-[var(--vz-duration-fast)]",
                // The prompt being read is lit in the live emerald; a hovered mark in the text colour.
                mark.kind === "current" ? "bg-live" : lit ? "bg-fg" : "bg-faint",
              )}
              style={{ width, opacity: lit ? 1 : mark.opacity }}
            />
            {mark.number != null && (
              <span className="type-mono-xs ml-6 text-faint" style={{ opacity: Math.min(1, mark.opacity + 0.1), fontSize: 9 }}>
                {mark.number}
              </span>
            )}
          </button>
        );
      })}
      {windowed && (
        <span className="type-mono-xs pointer-events-none absolute left-0 whitespace-nowrap text-faint" style={{ top: height + 6 }}>
          {count(chosen + 1)} / {count(total)}
        </span>
      )}
      {label && shownMark && (
        <span
          role="tooltip"
          className="pointer-events-none absolute z-10 flex h-24 w-max max-w-360 items-center gap-10 rounded-sm border border-border-strong bg-raised px-8 shadow-popover"
          style={{ left: windowed ? 44 : 22, top: middle + shownMark.offset * STEP - 12 }}
        >
          <span className={cx("type-small min-w-0 truncate", label.text ? "text-fg" : "text-faint")}>{label.text || "No text"}</span>
          {label.time && <span className="type-mono-xs shrink-0 text-faint">{label.time}</span>}
        </span>
      )}
    </div>
  );
}
