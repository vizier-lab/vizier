import type { HTMLAttributes, ReactNode } from "react";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";
import { NavItem } from "../NavItem/NavItem";

/** The agent's essentials, always one click away; everything else lives in the agent card's menu. */
export type EssentialPage = "memory" | "tasks" | "skills" | "dreams";

/** The pages behind the agent card, in the menu's three groups. */
export type MenuPage = "core" | "channels" | "addons" | "mcp" | "usage" | "secrets" | "config";

export const ESSENTIALS: { page: EssentialPage; label: string; icon: IconName }[] = [
  { page: "memory", label: "Memory", icon: "memory" },
  { page: "tasks", label: "Tasks", icon: "tasks" },
  { page: "skills", label: "Skills", icon: "skills" },
  { page: "dreams", label: "Dreams", icon: "dreams" },
];

export const MENU_GROUPS: { label: string; pages: { page: MenuPage; label: string; icon: IconName }[] }[] = [
  { label: "The agent", pages: [{ page: "core", label: "Core", icon: "core" }] },
  {
    label: "Connections",
    pages: [
      { page: "channels", label: "Channels", icon: "channels" },
      { page: "addons", label: "Addons", icon: "addons" },
      { page: "mcp", label: "MCP", icon: "mcp" },
    ],
  },
  {
    label: "Running it",
    pages: [
      { page: "usage", label: "Usage", icon: "usage" },
      { page: "secrets", label: "Secrets", icon: "key" },
      { page: "config", label: "Config", icon: "agent" },
    ],
  },
];

export type ContextSidebarProps = {
  /** The SidebarAgent (inside its menu's popover). */
  agent: ReactNode;
  active?: EssentialPage;
  /** Pages the person may not see. */
  hiddenPages?: EssentialPage[];
  hrefFor: (page: EssentialPage) => string;
  /** Below the essentials: the session list. It takes the rest of the height and scrolls itself. */
  children?: ReactNode;
} & HTMLAttributes<HTMLElement>;

/** The 208px column beside the rail: the agent, its essentials, and what the page puts below them. */
export function ContextSidebar({ agent, active, hiddenPages, hrefFor, children, className, ...rest }: ContextSidebarProps) {
  const hidden = new Set(hiddenPages);
  return (
    <nav aria-label="Agent" className={cx("flex h-full w-256 shrink-0 flex-col gap-8 bg-shell px-10 pt-12 pb-14", className)} {...rest}>
      {agent}
      <div className="flex flex-col gap-2 pt-4">
        {ESSENTIALS.filter((p) => !hidden.has(p.page)).map((p) => (
          <NavItem key={p.page} icon={p.icon} label={p.label} href={hrefFor(p.page)} active={active === p.page} />
        ))}
      </div>
      {children}
    </nav>
  );
}
