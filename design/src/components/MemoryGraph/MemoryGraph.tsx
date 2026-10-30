import { useEffect, useMemo, useRef, useState, type HTMLAttributes, type PointerEvent, type ReactNode, type WheelEvent } from "react";
import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  forceX,
  forceY,
  type Simulation,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from "d3-force";
import { cx } from "../../lib/cx";
import { FilterTabs } from "../FilterTabs/FilterTabs";
import { IconButton, IconButtonGroup } from "../IconButton/IconButton";
import { Slider } from "../Slider/Slider";
import { SwitchRow } from "../SwitchRow/SwitchRow";
import { Button } from "../Button/Button";

export type GraphNode = {
  id: string;
  label: string;
  /** Links (documents) or documents (bundles): sizes the star. */
  weight: number;
  /** Two hops away: drawn faded. */
  faded?: boolean;
  /** A starting position, 0–1 across and down; the simulation takes it from there. */
  x?: number;
  y?: number;
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
  /** A click on a star. Clicking empty space clears the selection (`null`). */
  onSelect?: (id: string | null) => void;
  /** A double-click: open the document or bundle. */
  onOpen?: (id: string) => void;
  /** Under the graph: what size and lines mean, and how much is shown. */
  caption?: ReactNode;
  /** Top left: the bundle graph's Largest · Recently changed · Most linked switch. */
  controls?: ReactNode;
  /** Starting values of the control panel. */
  defaultSettings?: Partial<GraphSettings>;
} & Omit<HTMLAttributes<HTMLDivElement>, "onSelect">;

/** How the layout behaves and how the graph is drawn; the control panel edits these. */
export type GraphSettings = {
  /** `auto` hides labels too small to read; `all` keeps every label; `none` only the hovered or selected one. */
  labels: "auto" | "all" | "none";
  /** Resting length of a link, in graph units. */
  linkDistance: number;
  /** How hard stars push each other apart. */
  repulsion: number;
  /** How strongly stars are drawn back to the middle. */
  centerPull: number;
  /** Scales every star. */
  nodeSize: number;
  /** Stop the layout where it is; stars stay put until moved by hand. */
  frozen: boolean;
};

export const DEFAULT_GRAPH_SETTINGS: GraphSettings = {
  labels: "auto",
  linkDistance: 110,
  repulsion: 260,
  centerPull: 0.03,
  nodeSize: 1,
  frozen: false,
};

type SimNode = SimulationNodeDatum & GraphNode;
type SimLink = SimulationLinkDatum<SimNode> & { dashed?: boolean };
type View = { x: number; y: number; k: number };
type Drag =
  | { kind: "pan"; startX: number; startY: number; from: View; moved: boolean }
  | { kind: "node"; node: SimNode; moved: boolean };

const MIN_ZOOM = 0.1;
const MAX_ZOOM = 5;
/** A label's size in graph units, so it grows and shrinks with the zoom. */
const LABEL_SIZE = 11.5;
/** Below this many pixels on screen a label is unreadable, so it is left out. */
const MIN_LABEL_PX = 7;

/**
 * A memory neighbourhood (X73), live: a force layout you can drag stars around
 * in, pan by dragging the background, and zoom with the wheel or the buttons.
 * Hovering a star lights its links; click selects, double-click opens.
 */
export function MemoryGraph({ nodes, edges, selectedId, onSelect, onOpen, caption, controls, defaultSettings, className, ...rest }: MemoryGraphProps) {
  const box = useRef<HTMLDivElement>(null);
  const svg = useRef<SVGSVGElement>(null);
  const sim = useRef<Simulation<SimNode, SimLink> | null>(null);
  const simNodes = useRef<SimNode[]>([]);
  const drag = useRef<Drag | null>(null);
  const justDragged = useRef(false);
  const [size, setSize] = useState({ w: 800, h: 600 });
  const [view, setView] = useState<View>({ x: 0, y: 0, k: 1 });
  const [, setTick] = useState(0);
  const [hover, setHover] = useState<string | null>(null);
  const [settings, setSettings] = useState<GraphSettings>({ ...DEFAULT_GRAPH_SETTINGS, ...defaultSettings });
  const [panel, setPanel] = useState(false);
  const set = <K extends keyof GraphSettings>(key: K, value: GraphSettings[K]) => setSettings((s) => ({ ...s, [key]: value }));
  // The simulation is built once per graph; the live settings are read through here.
  const live = useRef(settings);
  live.current = settings;

  // Follow the container's size.
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const obs = new ResizeObserver(([e]) => setSize({ w: e.contentRect.width || 800, h: e.contentRect.height || 600 }));
    obs.observe(el);
    return () => obs.disconnect();
  }, []);

  const max = Math.max(1, ...nodes.map((n) => n.weight));
  const radius = (n: GraphNode) => (3 + (n.weight / max) * 14) * settings.nodeSize;

  // (Re)build the simulation when the graph changes, keeping known positions.
  useEffect(() => {
    const prev = new Map(simNodes.current.map((n) => [n.id, n]));
    const { w, h } = size;
    const next: SimNode[] = nodes.map((n) => {
      const old = prev.get(n.id);
      return {
        ...n,
        x: old?.x ?? (n.x !== undefined ? n.x * w : w / 2 + (Math.random() - 0.5) * 40),
        y: old?.y ?? (n.y !== undefined ? n.y * h : h / 2 + (Math.random() - 0.5) * 40),
      };
    });
    simNodes.current = next;
    const ids = new Set(next.map((n) => n.id));
    const links: SimLink[] = edges.filter((e) => ids.has(e.from) && ids.has(e.to)).map((e) => ({ source: e.from, target: e.to, dashed: e.dashed }));
    sim.current?.stop();
    let frame = 0;
    const s = forceSimulation<SimNode, SimLink>(next)
      .force("link", forceLink<SimNode, SimLink>(links).id((d) => d.id).distance(live.current.linkDistance))
      .force("charge", forceManyBody().strength(-live.current.repulsion))
      .force("center", forceCenter(w / 2, h / 2).strength(live.current.centerPull + 0.02))
      .force("collide", forceCollide<SimNode>().radius((d) => radius(d) + 18))
      .force("x", forceX(w / 2).strength(live.current.centerPull))
      .force("y", forceY(h / 2).strength(live.current.centerPull))
      .alphaDecay(0.02)
      .velocityDecay(0.3)
      .on("tick", () => {
        // Keep every star, with its label to the right, inside the frame and clear of the caption.
        for (const n of next) {
          n.x = Math.max(24, Math.min(w - 130, n.x ?? w / 2));
          n.y = Math.max(52, Math.min(h - 44, n.y ?? h / 2));
        }
        cancelAnimationFrame(frame);
        frame = requestAnimationFrame(() => setTick((t) => t + 1));
      });
    // Under reduced motion, settle at once instead of animating.
    if (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) {
      s.stop();
      s.tick(300);
      setTick((t) => t + 1);
    }
    if (live.current.frozen) s.stop();
    sim.current = s;
    return () => {
      s.stop();
      cancelAnimationFrame(frame);
    };
  }, [nodes, edges, size.w, size.h]);

  // A changed setting reshapes the running layout instead of rebuilding it.
  useEffect(() => {
    const s = sim.current;
    if (!s) return;
    (s.force("link") as ReturnType<typeof forceLink<SimNode, SimLink>>).distance(settings.linkDistance);
    (s.force("charge") as ReturnType<typeof forceManyBody>).strength(-settings.repulsion);
    (s.force("center") as ReturnType<typeof forceCenter>).strength(settings.centerPull + 0.02);
    (s.force("x") as ReturnType<typeof forceX>).strength(settings.centerPull);
    (s.force("y") as ReturnType<typeof forceY>).strength(settings.centerPull);
    (s.force("collide") as ReturnType<typeof forceCollide<SimNode>>).radius((d) => radius(d) + 18);
    if (settings.frozen) s.stop();
    else s.alpha(0.6).restart();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings]);

  const toGraph = (clientX: number, clientY: number) => {
    const r = svg.current!.getBoundingClientRect();
    return { x: (clientX - r.left - view.x) / view.k, y: (clientY - r.top - view.y) / view.k };
  };

  const zoomAt = (k: number, px: number, py: number) => {
    setView((v) => {
      const nk = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, k));
      const ratio = nk / v.k;
      return { k: nk, x: px - (px - v.x) * ratio, y: py - (py - v.y) * ratio };
    });
  };

  const onWheel = (e: WheelEvent<SVGSVGElement>) => {
    const r = svg.current!.getBoundingClientRect();
    zoomAt(view.k * (e.deltaY > 0 ? 0.9 : 1.1), e.clientX - r.left, e.clientY - r.top);
  };

  // Escape closes the panel from anywhere on the page, not only from inside it.
  useEffect(() => {
    if (!panel) return;
    const close = (e: globalThis.KeyboardEvent) => e.key === "Escape" && setPanel(false);
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, [panel]);

  // The wheel must not scroll the page while it zooms the graph.
  useEffect(() => {
    const el = svg.current;
    if (!el) return;
    const stop = (e: globalThis.WheelEvent) => e.preventDefault();
    el.addEventListener("wheel", stop, { passive: false });
    return () => el.removeEventListener("wheel", stop);
  }, []);

  // A drag follows the pointer anywhere on the page until it is released.
  const startDrag = (e: PointerEvent<Element>, node?: SimNode) => {
    if (e.button !== 0) return;
    if (node) {
      node.fx = node.x;
      node.fy = node.y;
      drag.current = { kind: "node", node, moved: false };
    } else {
      drag.current = { kind: "pan", startX: e.clientX, startY: e.clientY, from: view, moved: false };
    }
    const move = (ev: globalThis.PointerEvent) => {
      const d = drag.current;
      if (!d) return;
      d.moved = true;
      if (d.kind === "pan") {
        setView({ ...d.from, x: d.from.x + ev.clientX - d.startX, y: d.from.y + ev.clientY - d.startY });
      } else {
        const p = toGraph(ev.clientX, ev.clientY);
        d.node.fx = p.x;
        d.node.fy = p.y;
        if (live.current.frozen) {
          // A frozen layout has no ticks to move the star, so place it directly.
          d.node.x = p.x;
          d.node.y = p.y;
          setTick((t) => t + 1);
        } else sim.current?.alpha(0.1).restart();
      }
    };
    const up = () => {
      const d = drag.current;
      if (d?.kind === "node") {
        d.node.fx = null;
        d.node.fy = null;
      }
      if (d?.moved) justDragged.current = true;
      drag.current = null;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
  };

  const select = (id: string | null) => {
    if (justDragged.current) {
      justDragged.current = false;
      return;
    }
    onSelect?.(id);
  };

  // Rebuilt with the simulation's node list; positions are read live from the nodes.
  const byId = useMemo(() => new Map(simNodes.current.map((n) => [n.id, n])), [simNodes.current]);
  const focus = hover ?? selectedId;
  const near = (e: GraphEdge) => e.from === focus || e.to === focus;
  const neighbours = new Set(edges.filter(near).flatMap((e) => [e.from, e.to]));
  const labelsReadable = settings.labels === "all" || (settings.labels === "auto" && LABEL_SIZE * view.k >= MIN_LABEL_PX);

  return (
    <div ref={box} className={cx("relative flex flex-col overflow-hidden rounded-xl", className)} {...rest}>
      {controls && <div className="absolute top-12 left-12 z-10">{controls}</div>}
      <IconButtonGroup aria-label="Zoom" className="absolute top-12 right-12 z-10">
        <IconButton variant="ghost" icon="zoom-in" aria-label="Zoom in" onClick={() => zoomAt(view.k * 1.3, size.w / 2, size.h / 2)} />
        <IconButton variant="ghost" icon="zoom-out" aria-label="Zoom out" onClick={() => zoomAt(view.k / 1.3, size.w / 2, size.h / 2)} />
        <IconButton variant="ghost" icon="maximize" aria-label="Reset view" onClick={() => setView({ x: 0, y: 0, k: 1 })} />
        <IconButton variant="ghost" icon="settings" aria-label="Graph settings" aria-expanded={panel} selected={panel} onClick={() => setPanel((p) => !p)} />
      </IconButtonGroup>
      {panel && (
        <div
          role="group"
          aria-label="Graph settings"
          className="absolute top-52 right-12 z-10 flex w-264 max-w-[calc(100%-24px)] flex-col gap-14 rounded-lg border border-border-strong bg-surface p-14 shadow-lg"
        >
          <div className="-mt-4 -mr-6 flex items-center justify-between gap-8">
            <span className="type-body-strong text-fg">Graph settings</span>
            <IconButton variant="ghost" size="sm" icon="close" aria-label="Close graph settings" onClick={() => setPanel(false)} />
          </div>
          <div className="flex flex-col gap-6">
            <span className="type-ui-medium text-soft">Labels</span>
            <FilterTabs
              aria-label="Labels"
              value={settings.labels}
              onChange={(v) => set("labels", v)}
              options={[
                { value: "auto", label: "Auto" },
                { value: "all", label: "All" },
                { value: "none", label: "None" },
              ]}
            />
          </div>
          <Slider label="Star size" min={0.5} max={2} step={0.1} value={settings.nodeSize} display={`${settings.nodeSize.toFixed(1)}×`} onChange={(v) => set("nodeSize", v)} />
          <Slider label="Link length" min={30} max={300} step={10} value={settings.linkDistance} onChange={(v) => set("linkDistance", v)} />
          <Slider label="Repulsion" min={50} max={800} step={10} value={settings.repulsion} onChange={(v) => set("repulsion", v)} />
          <Slider label="Pull to centre" min={0} max={0.2} step={0.01} value={settings.centerPull} display={settings.centerPull.toFixed(2)} onChange={(v) => set("centerPull", v)} />
          <SwitchRow label="Freeze layout" helper="Stars stay where they are until you move them." checked={settings.frozen} onCheckedChange={(v) => set("frozen", v)} />
          <Button size="sm" variant="ghost" onClick={() => setSettings({ ...DEFAULT_GRAPH_SETTINGS, ...defaultSettings })}>
            Reset settings
          </Button>
        </div>
      )}
      <svg
        ref={svg}
        width={size.w}
        height={size.h}
        role="group"
        aria-label="Memory graph. Drag a star to move it, drag the background to pan, scroll to zoom."
        className={cx("h-full w-full touch-none select-none", drag.current?.kind === "pan" ? "cursor-grabbing" : "cursor-grab")}
        onWheel={onWheel}
        onPointerDown={(e) => startDrag(e)}
        onClick={() => {
          setPanel(false);
          select(null);
        }}
      >
        <g transform={`translate(${view.x} ${view.y}) scale(${view.k})`}>
          {edges.map((e, i) => {
            const a = byId.get(e.from);
            const b = byId.get(e.to);
            if (!a || !b || a.x == null || b.x == null) return null;
            const strong = near(e);
            return (
              <line
                key={i}
                x1={a.x}
                y1={a.y}
                x2={b.x}
                y2={b.y}
                strokeWidth={(strong ? 1.5 : 1) / view.k}
                strokeDasharray={e.dashed ? `${4 / view.k} ${4 / view.k}` : undefined}
                className={cx(strong ? "stroke-graph-edge-strong" : "stroke-graph-edge", (a.faded || b.faded) && !strong && "opacity-50")}
              />
            );
          })}
          {simNodes.current.map((n) => {
            if (n.x == null || n.y == null) return null;
            const selected = n.id === selectedId;
            const lit = n.id === focus || neighbours.has(n.id);
            const r = radius(n);
            return (
              <g
                key={n.id}
                role="button"
                tabIndex={0}
                aria-label={`${n.label}${selected ? ", selected" : ""}`}
                aria-pressed={selected}
                onPointerDown={(e) => {
                  e.stopPropagation();
                  startDrag(e, n);
                }}
                onPointerEnter={() => setHover(n.id)}
                onPointerLeave={() => setHover(null)}
                onClick={(e) => {
                  e.stopPropagation();
                  select(n.id);
                }}
                onDoubleClick={(e) => {
                  e.stopPropagation();
                  onOpen?.(n.id);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter") onOpen?.(n.id);
                  if (e.key === " ") {
                    e.preventDefault();
                    onSelect?.(n.id);
                  }
                }}
                className={cx("cursor-pointer outline-none [&:focus-visible>circle:first-child]:stroke-focus", n.faded && !lit && "opacity-50")}
              >
                <circle cx={n.x} cy={n.y} r={r + 6} strokeWidth={2 / view.k} className="fill-transparent stroke-transparent" />
                {selected && <circle cx={n.x} cy={n.y} r={r + 5} strokeWidth={1.5 / view.k} className="fill-none stroke-ink" />}
                <circle cx={n.x} cy={n.y} r={r} className={n.faded && !lit ? "fill-muted" : "fill-ink"} />
                {(labelsReadable || selected || n.id === hover) && (
                  // Zoomed out too far to read, only the hovered or selected star keeps its label, held at the smallest readable size.
                  <text
                    x={n.x + r + 8}
                    y={n.y + 4}
                    style={{ fontSize: labelsReadable ? `${LABEL_SIZE}px` : `calc(${MIN_LABEL_PX}px / ${view.k})` }}
                    className={cx("font-mono", selected || n.id === hover ? "fill-fg" : n.faded && !lit ? "fill-faint" : "fill-soft")}
                  >
                    {n.label}
                  </text>
                )}
              </g>
            );
          })}
        </g>
      </svg>
      {caption && <p className="pointer-events-none absolute bottom-12 left-16 type-mono-xs text-faint">{caption}</p>}
    </div>
  );
}
