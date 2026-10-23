import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { AgentSwitcher } from "../AgentSwitcher/AgentSwitcher";
import { UserMenu } from "../UserMenu/UserMenu";
import { Sidebar, type SidebarPage } from "./Sidebar";

const PAGES: SidebarPage[] = ["home", "chat", "core", "memory", "tasks", "skills", "dreams", "channels", "addons", "mcp", "usage", "config", "settings"];

const meta = {
  title: "Components/Shell/Sidebar",
  component: Sidebar,
  tags: ["approved"],
  parameters: { vizier: { id: "D1", replaces: "webui/app/root.tsx" }, layout: "fullscreen" },
  args: {
    agent: <AgentSwitcher agentId="archivist" name="Archivist" state="live" detail="replying" />,
    user: <UserMenu name="ganesha" role="superadmin" onSignOut={() => {}} />,
    active: "chat",
    chatLive: true,
    hrefFor: (p) => `#${p}`,
    onToggleTheme: () => {},
    onToggleCollapsed: () => {},
  },
  argTypes: { active: { control: "select", options: PAGES } },
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
        agent={<AgentSwitcher agentId="archivist" name="Archivist" state="live" detail="replying" collapsed={collapsed} />}
        user={<UserMenu name="ganesha" role="superadmin" collapsed={collapsed} onSignOut={() => {}} />}
      />
    );
  },
};

/** The 64px rail; hover an icon for its name. */
export const Collapsed: Story = {
  args: {
    collapsed: true,
    agent: <AgentSwitcher agentId="archivist" name="Archivist" state="live" detail="replying" collapsed />,
    user: <UserMenu name="ganesha" role="superadmin" collapsed onSignOut={() => {}} />,
  },
};

/** No page active, as on pages outside the sidebar. */
export const NoneActive: Story = { args: { active: undefined, chatLive: false } };
