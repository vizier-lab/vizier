import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { UserMenu } from "../UserMenu/UserMenu";
import { SettingsNav } from "../SettingsNav/SettingsNav";
import { AgentMenu } from "./AgentMenu";
import { ContextSidebar } from "./ContextSidebar";
import { Rail } from "./Rail";
import { SessionList, type SessionGroup } from "./SessionList";
import { ShellFrame } from "./ShellFrame";
import { SidebarAgent } from "./SidebarAgent";

const meta = {
  title: "Components/Shell/Frame",
  component: ShellFrame,
  tags: ["approved"],
  parameters: { vizier: { id: "D1", replaces: "Sidebar" }, layout: "fullscreen" },
} satisfies Meta<typeof ShellFrame>;

export default meta;
type Story = StoryObj<typeof meta>;

const AGENTS = [
  { agentId: "archivist", name: "Archivist", state: "running", href: "#archivist" },
  { agentId: "scout", name: "Scout", state: "live", href: "#scout" },
  { agentId: "quill", name: "Quill", state: "stopped", href: "#quill" },
  { agentId: "ledger", name: "Ledger", state: "retrying", href: "#ledger" },
] as const;

const SESSIONS: SessionGroup[] = [
  {
    label: "Today",
    items: [
      { key: "a", title: "vector-db-review", time: "12m", live: true, href: "#a", active: true },
      { key: "b", title: "release-notes-draft", time: "2h", href: "#b" },
    ],
  },
  { label: "Yesterday", items: [{ key: "c", title: "q4-spend-check", time: "18:40", href: "#c" }, { key: "d", title: "onboarding-flow", time: "11:02", href: "#d" }] },
  { label: "This week", items: [{ key: "e", title: "memory-cleanup", time: "Tue", href: "#e" }] },
];

function Demo({ view }: { view: "home" | "agent" | "settings" }) {
  const [theme, setTheme] = useState(0);
  const rail = (
    <Rail
      homeHref="#home"
      homeActive={view === "home"}
      agents={[...AGENTS]}
      activeAgentId={view === "agent" ? "archivist" : undefined}
      addHref="#new"
      settingsHref="#settings"
      settingsActive={view === "settings"}
      onToggleTheme={() => setTheme(theme + 1)}
      user={<UserMenu name="admin" onSignOut={() => {}} collapsed />}
    />
  );
  const sidebar =
    view === "agent" ? (
      <ContextSidebar
        active="memory"
        hrefFor={(p) => `#${p}`}
        agent={
          <AgentMenu
            trigger={<SidebarAgent agentId="archivist" name="Archivist" state="running" detail="replying" />}
            onSelect={() => {}}
            onSwitchAgent={() => {}}
            switchShortcut="Ctrl K"
          />
        }
      >
        <SessionList groups={SESSIONS} onNew={() => {}} showAll={{ label: "Show all 63", onSelect: () => {} }} />
      </ContextSidebar>
    ) : view === "settings" ? (
      <SettingsNav active="profile" hrefFor={(p) => `#${p}`} />
    ) : undefined;
  return (
    <ShellFrame rail={rail} sidebar={sidebar} sidebarKey={view}>
      <div className="grid flex-1 place-items-center text-faint type-mono-xs">page</div>
    </ShellFrame>
  );
}

export const Home: Story = { args: { rail: null, children: null }, render: () => <Demo view="home" /> };
export const AgentPage: Story = { args: { rail: null, children: null }, render: () => <Demo view="agent" /> };
export const Settings: Story = { args: { rail: null, children: null }, render: () => <Demo view="settings" /> };
