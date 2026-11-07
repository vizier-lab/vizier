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
import { Presence } from "../../lib/Motion";
import { RailAgentMenu, type RailAgentPage } from "./RailAgentMenu";
import { SHELL_SIDEBAR_ID } from "./ShellFrame";
import { StatusDot } from "./StatusDot";

export type RailAgent = {
  agentId: string;
  name: string;
  state: AgentState;
  href: string;
  /** The agent's own picture, when it has one. */
  imageSrc?: string;
  /** Being created: the tile is busy, not yet a link, and shows a scrim with a spinner. */
  pending?: boolean;
  /** Replies not yet read in this person's chats; a count on the tile's corner. */
  unread?: number;
  /** Pages this person may not see of this agent, in its right-click menu. */
  hiddenPages?: readonly RailAgentPage[];
  /** Shown there but not reachable. */
  disabledPages?: readonly RailAgentPage[];
};

export type RailWorkspace = {
  id: string;
  /** The tooltip and the accessible name. */
  name: string;
  /** One or two letters on the tile. */
  initials: string;
  href: string;
  /** Messages from others not yet read; a count on the tile's corner. */
  unread?: number;
  /** An agent is working in it: a dot on the tile's corner. */
  working?: boolean;
};

export type RailProps = {
  homeHref: string;
  homeActive?: boolean;
  /** One tile per Workspace the person is in, between Home and the agents, each with its own separator. */
  workspaces?: RailWorkspace[];
  /** The Workspace whose topics are open; its tile carries the marker. */
  activeWorkspaceId?: string;
  /** One tile per agent, in the order given. */
  agents: RailAgent[];
  /** The agent whose pages are open; its tile carries the marker. */
  activeAgentId?: string;
  /** Called with the agents' ids in their new order when a tile is dropped. Tiles can be dragged when it is set. */
  onReorder?: (agentIds: string[]) => void;
  /** The dashed "+" after the agents; omit it for people who may not create one. */
  addHref?: string;
  addActive?: boolean;
  /**
   * The same "+" as a button: the host decides what it opens (a chooser between a Workspace and an
   * agent, or straight to the one the person may create). Used in place of `addHref` when set.
   */
  onCreate?: () => void;
  /** The label of that "+": "New agent", "New workspace", or "Create". */
  createLabel?: string;
  settingsHref: string;
  settingsActive?: boolean;
  /** The light and dark switch; omit it when the current look has only one of them. */
  onToggleTheme?: () => void;
  /** Shuts or opens the column beside the rail; omit it on pages that have no column to shut. */
  sidebarToggle?: {
    collapsed: boolean;
    onToggle: () => void;
    /** Shown in the tooltip, e.g. "Ctrl B". */
    shortcut?: string;
  };
  /** Right-clicking an agent's tile opens a menu of its pages; omit it to leave the browser's own menu. */
  agentMenu?: {
    /** The page open, for the agent named by `activeAgentId`. */
    activePage?: RailAgentPage;
    onNew: (agentId: string) => void;
    onSelect: (agentId: string, page: RailAgentPage) => void;
  };
  /** The avatar menu at the foot. */
  user: ReactNode;
} & HTMLAttributes<HTMLElement>;



/**
 * A label to the right of a rail control. It is drawn on the body with fixed coordinates, because
 * the rail scrolls and the sidebar beside it would otherwise paint over it.
 */
function RailTip({ label, muted, children }: { label: string; /** Something open beside the control would sit under it. */ muted?: boolean; children: ReactNode }) {
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
        !muted &&
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

/** A count on a tile's top corner. The tile's accessible name says it too. */
function UnreadBadge({ count }: { count: number }) {
  return (
    <span
      aria-hidden
      className="absolute -top-4 -right-4 grid h-16 min-w-16 place-items-center rounded-pill bg-rank-text px-4 type-mono-xs-medium text-shell ring-2 ring-shell"
    >
      {count > 99 ? "99+" : count}
    </span>
  );
}

function RailLink({ label, href, active, marker, children, className, onClick, onContextMenu, menuOpen }: {
  onClick?: (e: MouseEvent) => void;
  onContextMenu?: (e: MouseEvent<HTMLElement>) => void;
  /** Its right-click menu is open. Left out for a link that has none. */
  menuOpen?: boolean;
  label: string;
  href: string;
  active?: boolean;
  marker?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <RailTip label={label} muted={menuOpen}>
      <a
        href={href}
        aria-label={label}
        aria-current={active ? "page" : undefined}
        aria-haspopup={onContextMenu ? "menu" : undefined}
        aria-expanded={onContextMenu ? !!menuOpen : undefined}
        draggable={false}
        onClick={onClick}
        onContextMenu={onContextMenu}
        className={cx(TILE, className)}
      >
        {active && marker && <Marker id={marker} />}
        {children}
      </a>
    </RailTip>
  );
}

function RailButton({ icon, label, tip, active, href, onSelect, expanded, controls }: {
  icon: IconName;
  label: string;
  /** The tooltip, when it says more than the name. */
  tip?: string;
  active?: boolean;
  href?: string;
  onSelect?: () => void;
  expanded?: boolean;
  controls?: string;
}) {
  const cls = cx(
    "grid size-42 shrink-0 place-items-center rounded-lg transition-colors duration-[var(--vz-duration-base)]",
    active ? "bg-ink text-ground" : "text-muted hover:bg-surface hover:text-fg",
  );
  return (
    <RailTip label={tip ?? label}>
      {href !== undefined ? (
        <a href={href} aria-label={label} aria-current={active ? "page" : undefined} className={cls}>
          <Icon name={icon} size={18} />
        </a>
      ) : (
        <button type="button" aria-label={label} aria-expanded={expanded} aria-controls={controls} onClick={onSelect} className={cls}>
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
  workspaces,
  activeWorkspaceId,
  agents,
  activeAgentId,
  onReorder,
  addHref,
  addActive,
  onCreate,
  createLabel = "Create",
  settingsHref,
  settingsActive,
  onToggleTheme,
  sidebarToggle,
  agentMenu,
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
  // Whose right-click menu is open, and the tile it hangs from.
  const [menuFor, setMenuFor] = useState<string>();
  const menuAnchor = useRef<HTMLElement | null>(null);
  const menuAgent = menuFor ? byId.get(menuFor) : undefined;
  const workspaceTile = (w: RailWorkspace) => (
    <RailLink
      key={w.id}
      label={w.unread ? `${w.name}, ${w.unread} unread` : w.name}
      href={w.href}
      active={w.id === activeWorkspaceId}
      marker={marker}
      className="rounded-md"
    >
      <span
        aria-hidden
        className={cx(
          "grid size-42 place-items-center rounded-md type-body-strong tracking-tight transition-colors duration-[var(--vz-duration-base)]",
          w.id === activeWorkspaceId ? "bg-ink text-ground" : "bg-raised text-soft hover:bg-pressed hover:text-fg",
        )}
      >
        {w.initials}
      </span>
      {w.unread ? <UnreadBadge count={w.unread} /> : null}
      {w.working && <span aria-hidden className="absolute -right-2 -bottom-2 size-10 rounded-pill bg-live ring-2 ring-shell" />}
    </RailLink>
  );
  const tile = (a: RailAgent) =>
    a.pending ? (
      // Not a link yet: there is nowhere to go until the agent exists. The scrim keeps its sigil
      // visible (it is already drawn from the id) while the spinner says the work is still running.
      <RailTip label={`Creating ${a.name}`}>
        <div
          aria-busy="true"
          aria-label={`Creating ${a.name}`}
          className={cx(TILE, "rounded-md border border-dashed border-border-strong")}
        >
          <Sigil size="rail" agentId={a.agentId} image={a.imageSrc} />
          <span aria-hidden className="absolute inset-0 grid place-items-center rounded-md bg-shell/70">
            <span className="size-24 animate-spin rounded-pill border-[3px] border-border-strong border-t-live motion-reduce:animate-none" />
          </span>
        </div>
      </RailTip>
    ) : (
      <RailLink
        label={a.unread ? `${a.name}, ${a.unread} unread` : a.name}
        href={a.href}
        active={a.agentId === activeAgentId}
        marker={marker}
        className="rounded-md hover:opacity-90"
        onClick={(e) => dragged.current && e.preventDefault()}
        menuOpen={agentMenu ? menuFor === a.agentId : undefined}
        onContextMenu={
          agentMenu
            ? (e) => {
                e.preventDefault();
                menuAnchor.current = e.currentTarget;
                setMenuFor(a.agentId);
              }
            : undefined
        }
      >
        <Sigil size="rail" agentId={a.agentId} image={a.imageSrc} live={a.state === "live" || a.state === "running"} />
        {a.unread ? <UnreadBadge count={a.unread} /> : null}
        <StatusDot state={a.state} />
      </RailLink>
    );
  return (
    <nav aria-label="Agents" className={cx("flex h-full w-66 shrink-0 flex-col items-center gap-10 bg-shell pt-16 pb-14", className)} {...rest}>
      <RailLink label="Home" href={homeHref} active={homeActive} marker={marker} className={cx("rounded-lg", homeActive ? "bg-surface" : "hover:bg-surface")}>
        <Mark size={26} title="" />
      </RailLink>
      <span aria-hidden className="h-px w-22 bg-border-hover" />
      {/* The list clips where it scrolls: this room keeps a first tile's badge, which stands 6px
          above the tile with its ring, inside it without moving anything. */}
      <div className="-mt-6 flex min-h-0 w-full flex-1 flex-col items-center gap-10 overflow-y-auto overflow-x-clip pt-6 [scrollbar-width:none]">
        {workspaces && workspaces.length > 0 && (
          <>
            <ul className="m-0 flex list-none flex-col items-center gap-10 p-0" aria-label="Workspaces">
              {workspaces.map((w) => (
                <li key={w.id} className="relative">
                  {workspaceTile(w)}
                </li>
              ))}
            </ul>
            <span aria-hidden className="h-px w-22 shrink-0 bg-border-hover" />
          </>
        )}
        {onReorder ? (
          <Reorder.Group axis="y" values={order} onReorder={setOrder} className="m-0 flex list-none flex-col items-center gap-10 p-0">
            {order.map((id) => {
              const a = byId.get(id);
              if (!a) return null;
              // A tile that is still being created is not dragged: it has no place in the order yet.
              if (a.pending) {
                return (
                  <li key={id} className="relative">
                    {tile(a)}
                  </li>
                );
              }
              return (
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
        {onCreate ? (
          <RailTip label={createLabel}>
            <button
              type="button"
              aria-label={createLabel}
              onClick={onCreate}
              className={cx(TILE, "border border-dashed border-border-strong text-muted hover:border-border-hover hover:text-fg")}
            >
              <Icon name="plus" size={16} />
            </button>
          </RailTip>
        ) : addHref !== undefined ? (
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
        ) : null}
      </div>
      <div className="flex flex-col items-center">
        {/* First in a group anchored to the foot, so Theme and Settings don't move when it comes and goes. */}
        {sidebarToggle && (
          <RailButton
            icon={sidebarToggle.collapsed ? "sidebar-expand" : "sidebar-collapse"}
            label={sidebarToggle.collapsed ? "Expand sidebar" : "Collapse sidebar"}
            tip={`${sidebarToggle.collapsed ? "Expand sidebar" : "Collapse sidebar"}${sidebarToggle.shortcut ? ` (${sidebarToggle.shortcut})` : ""}`}
            onSelect={sidebarToggle.onToggle}
            expanded={!sidebarToggle.collapsed}
            controls={SHELL_SIDEBAR_ID}
          />
        )}
        {onToggleTheme &&<RailButton icon="theme" label="Theme" onSelect={onToggleTheme} />}
        <RailButton icon="settings" label="Settings" href={settingsHref} active={settingsActive} />
      </div>
      <span aria-hidden className="h-px w-22 bg-border" />
      {user}
      <Presence>
        {agentMenu && menuAgent && (
          <RailAgentMenu
            key={menuAgent.agentId}
            anchor={menuAnchor}
            name={menuAgent.name}
            active={menuAgent.agentId === activeAgentId ? agentMenu.activePage : undefined}
            hiddenPages={menuAgent.hiddenPages}
            disabledPages={menuAgent.disabledPages}
            onNew={() => agentMenu.onNew(menuAgent.agentId)}
            onSelect={(page) => agentMenu.onSelect(menuAgent.agentId, page)}
            onClose={() => setMenuFor(undefined)}
          />
        )}
      </Presence>
    </nav>
  );
}
