import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { AgentSwitcher } from "../AgentSwitcher/AgentSwitcher";
import { UserMenu } from "../UserMenu/UserMenu";
import { Sidebar, type SidebarPage } from "./Sidebar";

const PAGES: SidebarPage[] = ["home", "chat", "core", "memory", "tasks", "skills", "dreams", "channels", "addons", "mcp", "secrets", "usage", "config", "settings"];

const meta = {
  title: "Components/Shell/Sidebar",
  component: Sidebar,
  tags: ["approved"],
  parameters: { vizier: { id: "D1", replaces: "webui/app/root.tsx" }, layout: "fullscreen" },
  args: {
    agent: <AgentSwitcher agentId="archivist" name="Archivist" state="running" detail="replying" />,
    user: <UserMenu name="ganesha" role="superadmin" onSignOut={() => {}} />,
    active: "chat",
    chatLive: true,
    hrefFor: (p) => `#${p}`,
    onToggleTheme: () => {},
    onToggleCollapsed: () => {},
  },
  argTypes: {
    active: { control: "select", options: PAGES },
    hiddenPages: { control: "multi-select", options: PAGES },
    disabledPages: { control: "multi-select", options: PAGES },
  },
  decorators: [(Story) => <div className="h-1024">{Story()}</div>],
} satisfies Meta<typeof Sidebar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: function Render(args) {
    const [collapsed, setCollapsed] = useState(false);
    return (
      <Sidebar
        {...args}
        collapsed={collapsed}
        onToggleCollapsed={() => setCollapsed(!collapsed)}
        agent={<AgentSwitcher agentId="archivist" name="Archivist" state="running" detail="replying" collapsed={collapsed} />}
        user={<UserMenu name="ganesha" role="superadmin" collapsed={collapsed} onSignOut={() => {}} />}
      />
    );
  },
};

/** The 64px rail; hover an icon for its name. */
export const Collapsed: Story = {
  args: {
    collapsed: true,
    agent: <AgentSwitcher agentId="archivist" name="Archivist" state="running" detail="replying" collapsed />,
    user: <UserMenu name="ganesha" role="superadmin" collapsed onSignOut={() => {}} />,
  },
};

/** No page active, as on pages outside the sidebar. */
export const NoneActive: Story = { args: { active: undefined, chatLive: false } };

/** A page the person may not open is missing from Misc; the rest stay put. */
export const WithoutSecrets: Story = {
  args: { hiddenPages: ["secrets"], chatLive: false },
  parameters: {
    docs: {
      description: {
        story: "Secrets is hidden unless the person may manage the selected agent's secrets.",
      },
    },
  },
};

const AGENT_PAGES: SidebarPage[] = ["chat", "core", "memory", "tasks", "skills", "dreams", "channels", "addons", "mcp", "usage", "config"];

/** No agent chosen yet: the agent's pages stay in place, greyed, and lead nowhere. */
export const NoAgent: Story = {
  args: {
    active: "home",
    chatLive: false,
    hiddenPages: ["secrets"],
    disabledPages: AGENT_PAGES,
    agent: <AgentSwitcher agentId="none" name="Choose an agent" state="stopped" />,
  },
};
