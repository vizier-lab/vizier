import type { HTMLAttributes, ReactElement } from "react";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";
import { NavItem } from "../NavItem/NavItem";
import { NavSection } from "../NavSection/NavSection";
import { Tooltip } from "../Tooltip/Tooltip";

export type SidebarPage =
  | "home" | "chat" | "core" | "memory" | "artifacts" | "tasks" | "skills" | "dreams"
  | "channels" | "addons" | "mcp" | "secrets" | "usage" | "config" | "settings";

const GROUPS: { label?: string; pages: { page: SidebarPage; label: string; icon: IconName }[] }[] = [
  {
    pages: [
      { page: "home", label: "Home", icon: "home" },
      { page: "chat", label: "Chat", icon: "chat" },
      { page: "core", label: "Core", icon: "core" },
      { page: "memory", label: "Memory", icon: "memory" },
      { page: "artifacts", label: "Artifacts", icon: "folder" },
      { page: "tasks", label: "Tasks", icon: "tasks" },
      { page: "skills", label: "Skills", icon: "skills" },
      { page: "dreams", label: "Dreams", icon: "dreams" },
    ],
  },
  {
    label: "Integrations",
    pages: [
      { page: "channels", label: "Channels", icon: "channels" },
      { page: "addons", label: "Addons", icon: "addons" },
      { page: "mcp", label: "MCP", icon: "mcp" },
    ],
  },
  {
    label: "Misc",
    pages: [
      { page: "usage", label: "Usage", icon: "usage" },
      { page: "secrets", label: "Secrets", icon: "key" },
      { page: "config", label: "Config", icon: "agent" },
    ],
  },
];

export type SidebarProps = {
  /** The AgentSwitcher (or the agent Picker with it as trigger). */
  agent: ReactElement;
  /** The UserMenu at the foot. */
  user: ReactElement;
  /** The current page; none for pages outside the sidebar. */
  active?: SidebarPage;
  /** The 64px rail: icons only, with their names as tooltips. */
  collapsed?: boolean;
  /** Chat shows the live ring while the agent replies. */
  chatLive?: boolean;
  /** Pages the person may not see here, dropped from every group before rendering. */
  hiddenPages?: SidebarPage[];
  /** Pages shown but not reachable, e.g. the agent's pages while no agent is chosen. */
  disabledPages?: SidebarPage[];
  hrefFor: (page: SidebarPage) => string;
  onToggleTheme: () => void;
  onToggleCollapsed: () => void;
} & HTMLAttributes<HTMLElement>;

/** The app shell's sidebar (D1). No brand: it starts with the agent. */
export function Sidebar({
  agent,
  user,
  active,
  collapsed,
  chatLive,
  hiddenPages,
  disabledPages,
  hrefFor,
  onToggleTheme,
  onToggleCollapsed,
  className,
  ...rest
}: SidebarProps) {
  const wrap = (label: string, node: ReactElement<{ "aria-describedby"?: string }>) =>
    collapsed ? (
      <Tooltip key={label} label={label} side="right">
        {node}
      </Tooltip>
    ) : (
      node
    );
  // A page the person may not open is dropped from its group; a group left with
  // no pages loses its label too, rather than showing an empty heading.
  const hidden = new Set(hiddenPages ?? []);
  const groups = GROUPS.map((g) => ({ ...g, pages: g.pages.filter((p) => !hidden.has(p.page)) })).filter(
    (g) => g.pages.length > 0,
  );
  const disabled = new Set(disabledPages ?? []);
  const item = (p: { page: SidebarPage; label: string; icon: IconName }) =>
    wrap(
      p.label,
      <NavItem
        key={p.page}
        icon={p.icon}
        label={p.label}
        href={disabled.has(p.page) ? undefined : hrefFor(p.page)}
        disabled={disabled.has(p.page)}
        active={active === p.page}
        collapsed={collapsed}
        live={p.page === "chat" && chatLive}
      />,
    );
  return (
    <nav
      aria-label="Main"
      className={cx(
        // The rail eases between its two widths; the labels are clipped by the
        // shrinking edge rather than popped out of existence.
        "flex h-full flex-col gap-4 bg-sidebar pb-16 transition-[width] duration-[var(--vz-duration-slow)] ease-standard",
        // The rail has no edge line (with it the 44px tiles no longer fit the 64px width) and does
        // not clip, so the tooltips of its icons can reach out over the page.
        collapsed ? "w-64 items-center px-10" : "w-240 overflow-hidden border-r border-border px-14",
        className,
      )}
      {...rest}
    >
      {/* The same height as a page's header band, so the switcher lines up with the title. */}
      <div className={cx("flex h-76 shrink-0 items-center", collapsed ? "justify-center" : "w-full [&>*]:w-full")}>{agent}</div>
      {groups.map((g, i) =>
        g.label ? (
          <NavSection key={g.label} label={g.label} collapsed={collapsed}>
            {g.pages.map(item)}
          </NavSection>
        ) : (
          <div key={i} className="flex flex-col gap-4">
            {g.pages.map(item)}
          </div>
        ),
      )}
      <span className="flex-1" />
      {!hidden.has("settings") &&
        wrap("Settings", <NavItem icon="settings" label="Settings" href={hrefFor("settings")} active={active === "settings"} collapsed={collapsed} />)}
      {wrap("Theme", <NavItem icon="theme" label="Theme" onSelect={onToggleTheme} collapsed={collapsed} />)}
      {wrap(
        collapsed ? "Expand sidebar" : "Collapse sidebar",
        <NavItem
          icon="sidebar-collapse"
          label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          onSelect={onToggleCollapsed}
          collapsed={collapsed}
          className={collapsed ? "[&_svg]:rotate-180" : undefined}
        />,
      )}
      {user}
    </nav>
  );
}
