import type { ReactNode } from "react";
import { cx } from "../../lib/cx";

export type DayBlock = {
  key: string;
  /** Minutes after midnight. */
  minutes: number;
  node: ReactNode;
};

export type DayTimelineProps = {
  label: string;
  /** First and last hour drawn; the card scrolls if the window is taller than its space. */
  fromHour?: number;
  toHour?: number;
  blocks: DayBlock[];
  /** Minutes after midnight, and the words on the marker ("12:04"). Left out on any other day. */
  now?: { minutes: number; label: string };
  /** How many blocks share a row before the rest fold into a count. */
  maxColumns?: number;
  /** Caption of the folded count, e.g. (3) => "+3 more". */
  moreLabel?: (count: number) => string;
  onOpenMore?: (keys: string[]) => void;
  /** The line in the header strip. */
  caption?: ReactNode;
};

const ROW = 44;
const BLOCK_MINUTES = Math.round((40 / ROW) * 60);
const pad = (n: number) => String(n).padStart(2, "0");

/** Groups blocks whose moments would overlap on screen, in time order. */
export function clusterBlocks(blocks: DayBlock[]): DayBlock[][] {
  const sorted = [...blocks].sort((a, b) => a.minutes - b.minutes || a.key.localeCompare(b.key));
  const groups: DayBlock[][] = [];
  let end = -Infinity;
  for (const b of sorted) {
    if (b.minutes < end) groups[groups.length - 1].push(b);
    else groups.push([b]);
    end = Math.max(end, b.minutes + BLOCK_MINUTES);
  }
  return groups;
}

/** One day as hour rows with blocks at their moment (D53). Blocks mark a moment, never a span. */
export function DayTimeline({
  label,
  fromHour = 7,
  toHour = 21,
  blocks,
  now,
  maxColumns = 3,
  moreLabel = (n) => `+${n} more`,
  onOpenMore,
  caption,
}: DayTimelineProps) {
  const hours = Array.from({ length: toHour - fromHour + 1 }, (_, i) => fromHour + i);
  const y = (minutes: number) => ((minutes - fromHour * 60) / 60) * ROW;
  const visible = blocks.filter((b) => b.minutes >= fromHour * 60 && b.minutes < (toHour + 1) * 60);
  const nowY = now && now.minutes >= fromHour * 60 && now.minutes <= (toHour + 1) * 60 ? y(now.minutes) : null;
  return (
    <div role="group" aria-label={label} className="flex min-h-0 flex-col overflow-hidden rounded-xl border border-border bg-surface">
      {caption && <div className="type-mono-xs shrink-0 border-b border-border bg-sidebar px-16 py-10 text-faint">{caption}</div>}
      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="relative" style={{ height: hours.length * ROW }}>
          {hours.map((h, i) => (
            <div key={h} className={cx("absolute inset-x-0 flex", i > 0 && "border-t border-border")} style={{ top: i * ROW, height: ROW }}>
              <span className={cx("type-mono-xs w-64 shrink-0 px-12 pt-4 text-faint", nowY !== null && Math.abs(nowY - i * ROW) < 12 && "invisible")}>
                {pad(h)}:00
              </span>
            </div>
          ))}
          {clusterBlocks(visible).flatMap((group) => {
            const shown = group.length > maxColumns ? group.slice(0, maxColumns - 1) : group;
            const rest = group.length > maxColumns ? group.slice(maxColumns - 1) : [];
            const cols = shown.length + (rest.length ? 1 : 0);
            const cell = (i: number, top: number, key: string, node: ReactNode) => (
              <div
                key={key}
                className="absolute"
                style={{ top: top + 2, height: 40, left: `calc(64px + (100% - 76px) * ${i / cols})`, width: `calc((100% - 76px) / ${cols} - 6px)` }}
              >
                {node}
              </div>
            );
            return [
              ...shown.map((b, i) => cell(i, y(b.minutes), b.key, b.node)),
              ...(rest.length
                ? [
                    cell(
                      shown.length,
                      y(rest[0].minutes),
                      `${rest[0].key}+more`,
                      <button
                        type="button"
                        onClick={() => onOpenMore?.(rest.map((r) => r.key))}
                        className="flex size-full flex-col items-start justify-center rounded-md border border-border-strong bg-raised px-10 text-left transition-colors hover:bg-pressed"
                      >
                        <span className="type-small-medium text-fg">{moreLabel(rest.length)}</span>
                        <span className="type-mono-xs text-faint">opens the list</span>
                      </button>,
                    ),
                  ]
                : []),
            ];
          })}
          {nowY !== null && now && (
            <>
              <div aria-hidden className="absolute right-0 left-64 h-px bg-live" style={{ top: nowY }} />
              <span
                className="type-mono-xs absolute left-8 -translate-y-1/2 rounded-pill bg-live px-6 py-2 text-ground"
                style={{ top: nowY }}
              >
                {now.label}
              </span>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
