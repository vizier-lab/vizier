import { useLayoutEffect, useRef, useState } from "react";
import { cx } from "../../lib/cx";

/** A bar is 2px wide with a 2px gap. */
const PITCH = 4;

/** `levels` stretched or squeezed to `n` bars, so the shape survives any width. */
export function resampleLevels(levels: number[], n: number): number[] {
  if (n <= 0 || levels.length === 0) return [];
  if (levels.length === n) return levels;
  return Array.from({ length: n }, (_, i) => {
    const at = n === 1 ? 0 : (i * (levels.length - 1)) / (n - 1);
    const lo = Math.floor(at);
    const hi = Math.min(levels.length - 1, lo + 1);
    return levels[lo] + (levels[hi] - levels[lo]) * (at - lo);
  });
}

/** The newest `n` levels, newest last, with silence before them while there are fewer. */
export function tailLevels(levels: number[], n: number): number[] {
  if (n <= 0) return [];
  const tail = levels.slice(-n);
  return [...Array<number>(n - tail.length).fill(0), ...tail];
}

/**
 * Bars for an audio level series (0–1); bars before `progress` (0–1) are lit. It fills the width
 * it is given, a bar every 4px, resampling `levels` to fit; with no room to grow it keeps one bar
 * per level. `live` draws a microphone as it is heard: one bar per level, the newest at the right,
 * scrolling left as levels arrive.
 */
export function Waveform({ levels, progress = 1, live, className }: { levels: number[]; progress?: number; live?: boolean; className?: string }) {
  const box = useRef<HTMLSpanElement>(null);
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const measure = () => setWidth(el.clientWidth);
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  const natural = Math.max(0, levels.length * PITCH - 2);
  const fit = width ? Math.max(1, Math.floor((width + 2) / PITCH)) : levels.length;
  const bars = live ? tailLevels(levels, fit) : width ? resampleLevels(levels, fit) : levels;
  return (
    <span
      ref={box}
      aria-hidden
      // The bars never size the row: it starts at one bar per level and grows into what is free.
      style={{ flexBasis: natural }}
      className={cx("flex h-18 min-w-0 shrink grow items-center gap-2 overflow-hidden", className)}
    >
      {bars.map((l, i) => (
        <span
          key={i}
          className={cx("w-2 shrink-0 rounded-pill", i / bars.length < progress ? (live ? "bg-fg" : "bg-soft") : "bg-unlit")}
          style={{ height: `${Math.max(12, Math.round(l * 100))}%` }}
        />
      ))}
    </span>
  );
}

/** A deterministic level series for stories and placeholders. */
export function sampleLevels(n: number, seed = 7) {
  let x = seed;
  return Array.from({ length: n }, () => {
    x = (x * 9301 + 49297) % 233280;
    return 0.2 + (x / 233280) * 0.8;
  });
}
