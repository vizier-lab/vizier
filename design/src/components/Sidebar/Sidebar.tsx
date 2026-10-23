import type { HTMLAttributes, ReactElement } from "react";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";
import { NavItem } from "../NavItem/NavItem";
import { NavSection } from "../NavSection/NavSection";
import { Tooltip } from "../Tooltip/Tooltip";

export type SidebarPage =
  | "home" | "chat" | "core" | "memory" | "tasks" | "skills" | "dreams"
  | "channels" | "addons" | "mcp" | "usage" | "config" | "settings";

const GROUPS: { label?: string; pages: { page: SidebarPage; label: string; icon: IconName }[] }[] = [
  {
    pages: [
      { page: "home", label: "Home", icon: "home" },
      { page: "chat", label: "Chat", icon: "chat" },
      { page: "core", label: "Core", icon: "core" },
      { page: "memory", label: "Memory", icon: "memory" },
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
  const item = (p: { page: SidebarPage; label: string; icon: IconName }) =>
    wrap(
      p.label,
      <NavItem
        key={p.page}
        icon={p.icon}
        label={p.label}
        href={hrefFor(p.page)}
        active={active === p.page}
        collapsed={collapsed}
        live={p.page === "chat" && chatLive}
      />,
    );
  return (
    <nav
      aria-label="Main"
      className={cx(
        "flex h-full flex-col gap-4 border-r border-border bg-sidebar pb-16",
        collapsed ? "w-64 items-center px-10" : "w-240 px-14",
        className,
      )}
      {...rest}
    >
      {/* The same height as a page's header band, so the switcher lines up with the title. */}
      <div className={cx("flex h-76 shrink-0 items-center", collapsed ? "justify-center" : "w-full [&>*]:w-full")}>{agent}</div>
      {GROUPS.map((g, i) =>
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
      {wrap("Settings", <NavItem icon="settings" label="Settings" href={hrefFor("settings")} active={active === "settings"} collapsed={collapsed} />)}
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
