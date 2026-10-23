import { useState, type HTMLAttributes, type ReactNode } from "react";
import { cx } from "../../lib/cx";
import { IconButton, IconButtonGroup } from "../IconButton/IconButton";

export type GraphNode = {
  id: string;
  label: string;
  /** Position, 0–1 across and down. Layout is the caller's (a force layout, or stored). */
  x: number;
  y: number;
  /** Links (documents) or documents (bundles): sizes the star. */
  weight: number;
  /** Two hops away: drawn faded. */
  faded?: boolean;
};

export type GraphEdge = {
  from: string;
  to: string;
  /** Into another bundle. */
  dashed?: boolean;
};

export type MemoryGraphProps = {
  nodes: GraphNode[];
  edges: GraphEdge[];
  selectedId?: string;
  onSelect?: (id: string) => void;
  /** Under the graph: what size and lines mean, and how much is shown. */
  caption?: ReactNode;
  /** Top left: the bundle graph's Largest · Recently changed · Most linked switch. */
  controls?: ReactNode;
} & Omit<HTMLAttributes<HTMLDivElement>, "onSelect">;

const W = 1000;
const H = 700;

/**
 * A memory neighbourhood (X73): stars sized by links, the selected one ringed,
 * its own links bright, second hops faded, links into other bundles dashed.
 */
export function MemoryGraph({ nodes, edges, selectedId, onSelect, caption, controls, className, ...rest }: MemoryGraphProps) {
  const [zoom, setZoom] = useState(1);
  const max = Math.max(1, ...nodes.map((n) => n.weight));
  const r = (n: GraphNode) => 3 + (n.weight / max) * 14;
  const at = new Map(nodes.map((n) => [n.id, n]));
  const touches = (e: GraphEdge) => e.from === selectedId || e.to === selectedId;
  return (
    <div className={cx("relative flex flex-col overflow-hidden rounded-xl bg-ground", className)} {...rest}>
      {controls && <div className="absolute top-12 left-12 z-10">{controls}</div>}
      <IconButtonGroup aria-label="Zoom" className="absolute top-12 right-12 z-10">
        <IconButton variant="ghost" icon="zoom-in" aria-label="Zoom in" onClick={() => setZoom((z) => Math.min(3, z * 1.25))} />
        <IconButton variant="ghost" icon="zoom-out" aria-label="Zoom out" onClick={() => setZoom((z) => Math.max(0.5, z / 1.25))} />
        <IconButton variant="ghost" icon="maximize" aria-label="Fit to view" onClick={() => setZoom(1)} />
      </IconButtonGroup>
      <svg viewBox={`0 0 ${W} ${H}`} className="h-full w-full" role="group" aria-label="Memory graph">
        <g transform={`translate(${(W * (1 - zoom)) / 2} ${(H * (1 - zoom)) / 2}) scale(${zoom})`}>
          {edges.map((e, i) => {
            const a = at.get(e.from);
            const b = at.get(e.to);
            if (!a || !b) return null;
            const strong = touches(e);
            return (
              <line
                key={i}
                x1={a.x * W}
                y1={a.y * H}
                x2={b.x * W}
                y2={b.y * H}
                strokeWidth={strong ? 1.5 : 1}
                strokeDasharray={e.dashed ? "4 4" : undefined}
                className={cx(strong ? "stroke-graph-edge-strong" : "stroke-graph-edge", (a.faded || b.faded) && !strong && "opacity-50")}
              />
            );
          })}
          {nodes.map((n) => {
            const selected = n.id === selectedId;
            return (
              <g
                key={n.id}
                role="button"
                tabIndex={0}
                aria-label={`${n.label}${selected ? ", selected" : ""}`}
                aria-pressed={selected}
                onClick={() => onSelect?.(n.id)}
                onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), onSelect?.(n.id))}
                className={cx("cursor-pointer outline-none [&:focus-visible>circle:first-child]:stroke-focus", n.faded && "opacity-50")}
              >
                <circle cx={n.x * W} cy={n.y * H} r={r(n) + 6} className="fill-transparent stroke-transparent" strokeWidth={2} />
                {selected && <circle cx={n.x * W} cy={n.y * H} r={r(n) + 5} className="fill-none stroke-ink" strokeWidth={1.5} />}
                <circle cx={n.x * W} cy={n.y * H} r={r(n)} className={n.faded ? "fill-muted" : "fill-ink"} />
                <text x={n.x * W + r(n) + 8} y={n.y * H + 4} className={cx("type-mono-sm", selected ? "fill-fg" : n.faded ? "fill-faint" : "fill-soft")}>
                  {n.label}
                </text>
              </g>
            );
          })}
        </g>
      </svg>
      {caption && <p className="absolute bottom-12 left-16 type-mono-xs text-faint">{caption}</p>}
    </div>
  );
}
