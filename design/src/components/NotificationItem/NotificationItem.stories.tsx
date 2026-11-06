import type { Meta, StoryObj } from "@storybook/react-vite";
import { NotificationItem } from "./NotificationItem";

const meta = {
  title: "Components/Lists/NotificationItem",
  component: NotificationItem,
  tags: ["approved"],
  parameters: {
    docs: {
      description: {
        component:
          "One notification in the popup: sigil, conversation title, what happened, a short excerpt, the time and an eye that toggles read. Unread is outlined in emerald.",
      },
    },
  },
  args: {
    agentId: "alpha",
    agentName: "Alpha",
    title: "Pricing page copy review",
    text: "Alpha replied",
    excerpt: "The middle plan is underpriced by about 12%, so I moved the annual discount to the Team tier and…",
    time: "2m",
    read: false,
    onOpen: () => {},
    onToggleRead: () => {},
  },
  argTypes: { tone: { control: "inline-radio", options: ["default", "failed", "success"] } },
  decorators: [(Story) => <div className="w-376">{Story()}</div>],
} satisfies Meta<typeof NotificationItem>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Unread: Story = {};

export const Read: Story = { args: { read: true } };

export const Failed: Story = {
  args: { agentId: "scout", agentName: "Scout", title: "Weekly competitor brief", text: "Scheduled run failed", excerpt: undefined, tone: "failed", time: "1h" },
};

/** A row from before titles existed, or one whose conversation is someone else's: the agent's name stands in. */
export const NoTitle: Story = { args: { title: undefined, excerpt: undefined } };

export const NoToggle: Story = { args: { onToggleRead: undefined } };
