/** Space between two marks, in pixels. */
export const STEP = 14;

/** Every this many prompts a longer mark, numbered once the ruler is windowed. */
export const MAJOR_EVERY = 10;

export type Mark = {
  /** 0-based index of the prompt. */
  index: number;
  /** Offset from the ruler's middle, in marks (negative is above). */
  offset: number;
  kind: "current" | "major" | "minor";
  /** 0..1; marks fade towards an edge that has more prompts beyond it. */
  opacity: number;
  /** The 1-based number written beside a major mark, when the ruler is windowed. */
  number?: number;
};

export type Window = {
  marks: Mark[];
  /** More prompts than fit: only a window is shown, and every tenth is numbered. */
  windowed: boolean;
};

/**
 * The marks of a window of `radius * 2 + 1` prompts around `center`, always full: near either
 * end the window stops at the first or last prompt instead of leaving half the ruler empty, so
 * the centre is then off the middle. `current` (the prompt being read) is the long mark wherever
 * it falls. Marks fade towards an edge only when more prompts lie beyond it.
 */
export function marksAround(total: number, center: number, current: number, radius: number): Window {
  if (total <= 0) return { marks: [], windowed: false };
  const size = radius * 2 + 1;
  const windowed = total > size;
  const first = windowed ? clamp(center - radius, 0, total - size) : 0;
  const last = windowed ? first + size - 1 : total - 1;
  // The middle of what is shown: the window's own centre, or the middle of a short list.
  const middle = (first + last) / 2;
  const moreAbove = first > 0;
  const moreBelow = last < total - 1;
  const marks: Mark[] = [];
  for (let index = first; index <= last; index += 1) {
    const offset = index - middle;
    const major = (index + 1) % MAJOR_EVERY === 0;
    const kind = index === current ? "current" : major ? "major" : "minor";
    // Distance from the window's edge on a side that continues, in marks.
    const fades = (offset < 0 && moreAbove) || (offset > 0 && moreBelow);
    const opacity = index === current ? 1 : fades ? Math.max(0.12, 0.85 - Math.abs(offset) / (radius + 1)) : windowed ? 0.7 : 0.5;
    marks.push({ index, offset, kind, opacity, number: windowed && major && index !== current ? index + 1 : undefined });
  }
  return { marks, windowed };
}

/** The prompt under a point `dy` pixels below the ruler's middle, or null between none. */
export function indexAt(marks: Mark[], dy: number): number | null {
  let best: Mark | null = null;
  for (const mark of marks) {
    if (Math.abs(mark.offset * STEP - dy) <= STEP / 2 && (!best || Math.abs(mark.offset * STEP - dy) < Math.abs(best.offset * STEP - dy))) best = mark;
  }
  return best?.index ?? null;
}

export const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));

/**
 * Turns wheel movement into whole prompts: one per notch however the device reports it, so a
 * trackpad's stream of small deltas does not skip dozens.
 */
export function wheelSteps(carry: number, deltaY: number, deltaMode: number): { steps: number; carry: number } {
  // Lines and pages are notches already; pixels are summed until one notch's worth.
  const NOTCH = 50;
  const px = deltaMode === 0 ? deltaY : Math.sign(deltaY) * NOTCH;
  const sum = carry + px;
  const steps = Math.trunc(sum / NOTCH);
  return { steps, carry: sum - steps * NOTCH };
}

/** "1,240", the way the counter writes numbers. */
export const count = (n: number) => n.toLocaleString("en-US");
