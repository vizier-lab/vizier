import { useEffect, useId, useRef, useState, type HTMLAttributes, type MouseEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { motion, Reorder } from "motion/react";
import { Mark } from "../../brand/Mark/Mark";
import { Sigil } from "../../brand/Sigil/Sigil";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";
import { motionDurations, motionEases } from "../../tokens/tokens";
import type { AgentState } from "../Status/Status";

export type RailAgent = {
  agentId: string;
  name: string;
  state: AgentState;
  href: string;
  /** The agent's own picture, when it has one. */
  imageSrc?: string;
};

export type RailProps = {
  homeHref: string;
  homeActive?: boolean;
  /** One tile per agent, in the order given. */
  agents: RailAgent[];
  /** The agent whose pages are open; its tile carries the marker. */
  activeAgentId?: string;
  /** Called with the agents' ids in their new order when a tile is dropped. Tiles can be dragged when it is set. */
  onReorder?: (agentIds: string[]) => void;
  /** The dashed "+" after the agents; omit it for people who may not create one. */
  addHref?: string;
  addActive?: boolean;
  settingsHref: string;
  settingsActive?: boolean;
  onToggleTheme: () => void;
  /** The avatar menu at the foot. */
  user: ReactNode;
} & HTMLAttributes<HTMLElement>;

/** The corner dot of an agent with a picture: live and running are emerald, a failing agent amber or red, the rest grey. */
const DOT: Record<AgentState, string> = {
  live: "bg-live",
  running: "bg-live",
  starting: "bg-muted",
  retrying: "bg-retrying",
  failed: "bg-failed",
  stopped: "bg-unlit",
};


/**
 * A label to the right of a rail control. It is drawn on the body with fixed coordinates, because
 * the rail scrolls and the sidebar beside it would otherwise paint over it.
 */
function RailTip({ label, children }: { label: string; children: ReactNode }) {
  const ref = useRef<HTMLSpanElement>(null);
  const [at, setAt] = useState<{ x: number; y: number } | null>(null);
  const show = () => {
    const r = ref.current?.getBoundingClientRect();
    if (r) setAt({ x: r.right + 8, y: r.top + r.height / 2 });
  };
  return (
    <span ref={ref} className="inline-flex" onMouseEnter={show} onMouseLeave={() => setAt(null)} onFocus={show} onBlur={() => setAt(null)}>
      {children}
      {at &&
        createPortal(
          <span
            role="tooltip"
            style={{ position: "fixed", left: at.x, top: at.y, transform: "translateY(-50%)", zIndex: 70 }}
            className="pointer-events-none rounded-md bg-ink px-8 py-4 type-small whitespace-nowrap text-ground shadow-popover"
          >
            {label}
          </span>,
          document.body,
        )}
    </span>
  );
}

const TILE = "relative grid size-42 shrink-0 place-items-center rounded-lg transition-colors duration-[var(--vz-duration-base)]";

/** The marker that tells which tile is open. One element shared by all tiles, so it slides. */
function Marker({ id }: { id: string }) {
  return (
    <motion.span
      layoutId={id}
      aria-hidden
      className="absolute top-1/2 -left-8 h-22 w-2 -translate-y-1/2 rounded-pill bg-fg"
      transition={{ duration: motionDurations.base, ease: motionEases.standard }}
    />
  );
}

function RailLink({ label, href, active, marker, children, className, onClick }: {
  onClick?: (e: MouseEvent) => void;
  label: string;
  href: string;
  active?: boolean;
  marker?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <RailTip label={label}>
      <a href={href} aria-label={label} aria-current={active ? "page" : undefined} draggable={false} onClick={onClick} className={cx(TILE, className)}>
        {active && marker && <Marker id={marker} />}
        {children}
      </a>
    </RailTip>
  );
}

function RailButton({ icon, label, active, href, onSelect }: { icon: IconName; label: string; active?: boolean; href?: string; onSelect?: () => void }) {
  const cls = cx(
    "grid size-42 shrink-0 place-items-center rounded-lg transition-colors duration-[var(--vz-duration-base)]",
    active ? "bg-ink text-ground" : "text-muted hover:bg-surface hover:text-fg",
  );
  return (
    <RailTip label={label}>
      {href !== undefined ? (
        <a href={href} aria-label={label} aria-current={active ? "page" : undefined} className={cls}>
          <Icon name={icon} size={18} />
        </a>
      ) : (
        <button type="button" aria-label={label} onClick={onSelect} className={cls}>
          <Icon name={icon} size={18} />
        </button>
      )}
    </RailTip>
  );
}

/**
 * The narrow strip at the left edge of the app: Home, an agent tile for each agent, and at the foot
 * the theme, settings and the person. Choosing a tile opens that agent's pages in the context
 * sidebar beside it.
 */
export function Rail({
  homeHref,
  homeActive,
  agents,
  activeAgentId,
  onReorder,
  addHref,
  addActive,
  settingsHref,
  settingsActive,
  onToggleTheme,
  user,
  className,
  ...rest
}: RailProps) {
  const marker = useId();
  // The order shown while a tile is dragged; it follows the agents given whenever they change.
  const ids = agents.map((a) => a.agentId);
  const [order, setOrder] = useState(ids);
  const key = ids.join("\u0000");
  useEffect(() => setOrder(ids), [key]); // eslint-disable-line react-hooks/exhaustive-deps
  // A drop is not a click: the link under the pointer must not open.
  const dragged = useRef(false);
  const byId = new Map(agents.map((a) => [a.agentId, a]));
  const tile = (a: RailAgent) => (
    <RailLink
      label={a.name}
      href={a.href}
      active={a.agentId === activeAgentId}
      marker={marker}
      className="rounded-md hover:opacity-90"
      onClick={(e) => dragged.current && e.preventDefault()}
    >
      <Sigil size="rail" agentId={a.agentId} image={a.imageSrc} live={a.state === "live" || a.state === "running"} />
      {/* The pattern shows the agent live by itself; a picture has nowhere to, so it gets the corner dot. */}
      {a.imageSrc && <span aria-hidden className={cx("absolute -right-3 -bottom-3 size-10 rounded-pill border-2 border-shell", DOT[a.state])} />}
    </RailLink>
  );
  return (
    <nav aria-label="Agents" className={cx("flex h-full w-66 shrink-0 flex-col items-center gap-10 bg-shell pt-16 pb-14", className)} {...rest}>
      <RailLink label="Home" href={homeHref} active={homeActive} marker={marker} className={cx("rounded-lg", homeActive ? "bg-surface" : "hover:bg-surface")}>
        <Mark size={26} title="" />
      </RailLink>
      <span aria-hidden className="h-px w-22 bg-border" />
      <div className="flex min-h-0 w-full flex-1 flex-col items-center gap-10 overflow-y-auto overflow-x-clip [scrollbar-width:none]">
        {onReorder ? (
          <Reorder.Group axis="y" values={order} onReorder={setOrder} className="m-0 flex list-none flex-col items-center gap-10 p-0">
            {order.map((id) => {
              const a = byId.get(id);
              return (
                a && (
                  <Reorder.Item
                    key={id}
                    value={id}
                    className="relative cursor-grab active:cursor-grabbing"
                    whileDrag={{ scale: 1.08, zIndex: 5 }}
                    onDragStart={() => (dragged.current = true)}
                    onDragEnd={() => {
                      onReorder(order);
                      setTimeout(() => (dragged.current = false), 0);
                    }}
                  >
                    {tile(a)}
                  </Reorder.Item>
                )
              );
            })}
          </Reorder.Group>
        ) : (
          <ul className="m-0 flex list-none flex-col items-center gap-10 p-0">
            {agents.map((a) => (
              <li key={a.agentId} className="relative">
                {tile(a)}
              </li>
            ))}
          </ul>
        )}
        {addHref !== undefined && (
          <RailLink
            label="New agent"
            href={addHref}
            active={addActive}
            className={cx(
              "border text-muted",
              addActive ? "border-ink bg-ink text-ground" : "border-border-strong hover:border-border-hover hover:text-fg",
            )}
          >
            <Icon name="plus" size={16} />
          </RailLink>
        )}
      </div>
      <div className="flex flex-col items-center">
        <RailButton icon="theme" label="Theme" onSelect={onToggleTheme} />
        <RailButton icon="settings" label="Settings" href={settingsHref} active={settingsActive} />
      </div>
      <span aria-hidden className="h-px w-22 bg-border" />
      {user}
    </nav>
  );
}
