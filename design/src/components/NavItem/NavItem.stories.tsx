import type { Meta, StoryObj } from "@storybook/react-vite";
import { StateGrid, interactionCells, statePseudo } from "../../stories/StateGrid";
import { NavItem } from "./NavItem";

const meta = {
  title: "Components/Shell/NavItem",
  component: NavItem,
  tags: ["approved"],
  parameters: { vizier: { id: "D2", replaces: null } },
  args: { icon: "home", label: "Home", href: "#", active: false, collapsed: false, live: false },
  decorators: [(Story) => <div className="w-212">{Story()}</div>],
} satisfies Meta<typeof NavItem>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Active: Story = { args: { active: true } };

export const Live: Story = { args: { icon: "chat", label: "Chat", live: true } };

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  render: () => (
    <StateGrid
      rows={[false, true].map((collapsed) => ({
        label: collapsed ? "collapsed" : "expanded",
        cells: interactionCells(
          () => (
            <div className={collapsed ? "" : "w-212"}>
              <NavItem icon="home" label="Home" href="#" collapsed={collapsed} />
            </div>
          ),
          [
            {
              label: "Active",
              node: (
                <div className={collapsed ? "" : "w-212"}>
                  <NavItem icon="home" label="Home" href="#" collapsed={collapsed} active />
                </div>
              ),
            },
          ],
        ),
      }))}
    />
  ),
};

/** Unread notifications: the count pill, or a dot in the rail. */
export const Unread: Story = { args: { icon: "bell", label: "Notifications", count: 3, href: undefined, onSelect: () => {} } };

/** Setup activity is a separate, pulsing dot — never part of the unread count. */
export const Busy: Story = { args: { icon: "bell", label: "Notifications", busy: true, href: undefined, onSelect: () => {} } };

export const UnreadAndBusy: Story = {
  args: { icon: "bell", label: "Notifications", count: 12, busy: true, href: undefined, onSelect: () => {} },
};

/** The rail: unread and busy become two dots on the bell. */
export const CollapsedUnreadAndBusy: Story = {
  args: { icon: "bell", label: "Notifications", count: 12, busy: true, collapsed: true, href: undefined, onSelect: () => {} },
  decorators: [(Story) => <div className="w-88">{Story()}</div>],
};
