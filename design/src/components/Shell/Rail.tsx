import { useId, useRef, useState, type HTMLAttributes, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { motion } from "motion/react";
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
};

export type RailProps = {
  homeHref: string;
  homeActive?: boolean;
  /** One tile per agent, in the order given. */
  agents: RailAgent[];
  /** The agent whose pages are open; its tile carries the marker. */
  activeAgentId?: string;
  /** The dashed "+" after the agents; omit it for people who may not create one. */
  addHref?: string;
  addActive?: boolean;
  settingsHref: string;
  settingsActive?: boolean;
  onToggleTheme: () => void;
  /** The avatar menu at the foot. */
  user: ReactNode;
} & HTMLAttributes<HTMLElement>;

/** The corner dot: live and running are emerald, a failing agent amber or red, the rest grey. */
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

function RailLink({ label, href, active, marker, children, className }: {
  label: string;
  href: string;
  active?: boolean;
  marker?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <RailTip label={label}>
      <a href={href} aria-label={label} aria-current={active ? "page" : undefined} className={cx(TILE, className)}>
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
  return (
    <nav aria-label="Agents" className={cx("flex h-full w-64 shrink-0 flex-col items-center gap-10 bg-shell pt-16 pb-14", className)} {...rest}>
      <RailButton icon="home" label="Home" href={homeHref} active={homeActive} />
      <span aria-hidden className="h-px w-22 bg-border" />
      <ul className="m-0 flex min-h-0 w-full flex-1 list-none flex-col items-center gap-10 overflow-y-auto overflow-x-clip p-0 [scrollbar-width:none]">
        {agents.map((a) => {
          const active = a.agentId === activeAgentId;
          return (
            <li key={a.agentId} className="relative">
              <RailLink label={a.name} href={a.href} active={active} marker={marker} className="rounded-md hover:opacity-90">
                <Sigil size="rail" agentId={a.agentId} live={a.state === "live" || a.state === "running"} />
                <span
                  aria-hidden
                  className={cx("absolute -right-3 -bottom-3 size-10 rounded-pill border-2 border-shell", DOT[a.state])}
                />
              </RailLink>
            </li>
          );
        })}
        {addHref !== undefined && (
          <li>
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
          </li>
        )}
      </ul>
      <RailButton icon="theme" label="Theme" onSelect={onToggleTheme} />
      <RailButton icon="settings" label="Settings" href={settingsHref} active={settingsActive} />
      {user}
    </nav>
  );
}
