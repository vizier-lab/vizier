import type { Meta, StoryObj } from "@storybook/react-vite";
import { NavItem } from "../NavItem/NavItem";
import { NavSection } from "./NavSection";

const meta = {
  title: "Components/Shell/NavSection",
  component: NavSection,
  tags: ["approved"],
  parameters: { vizier: { id: "D3", replaces: null } },
  args: { label: "Integrations", collapsed: false },
} satisfies Meta<typeof NavSection>;

export default meta;
type Story = StoryObj<typeof meta>;

const items = (collapsed?: boolean) => (
  <>
    <NavItem icon="channels" label="Channels" href="#" collapsed={collapsed} />
    <NavItem icon="addons" label="Addons" href="#" collapsed={collapsed} />
    <NavItem icon="mcp" label="MCP" href="#" collapsed={collapsed} />
  </>
);

export const Default: Story = {
  render: (args) => (
    <div className="w-212">
      <NavSection {...args}>{items(args.collapsed)}</NavSection>
    </div>
  ),
};

export const Collapsed: Story = {
  args: { collapsed: true },
  render: (args) => (
    <div className="w-64 px-10">
      <NavItem icon="home" label="Home" href="#" collapsed />
      <NavSection {...args}>{items(true)}</NavSection>
    </div>
  ),
};
