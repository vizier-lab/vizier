import type { Meta, StoryObj } from "@storybook/react-vite";
import { IconButton } from "../IconButton/IconButton";
import { Tooltip, TooltipBubble } from "./Tooltip";

const meta = {
  title: "Components/Feedback/Tooltip",
  component: Tooltip,
  tags: ["approved"],
  parameters: { vizier: { id: "D33", replaces: "webui/app/components/TooltipLabel.tsx" } },
  args: {
    label: "Collapse sidebar",
    shortcut: "Ctrl B",
    side: "right",
    children: <IconButton variant="ghost" icon="sidebar-collapse" aria-label="Collapse sidebar" />,
  },
  argTypes: { side: { control: "inline-radio", options: ["top", "bottom", "right", "left"] } },
  decorators: [(Story) => <div className="p-48">{Story()}</div>],
} satisfies Meta<typeof Tooltip>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Hover or tab to the button. */
export const Default: Story = {};

/** The bubbles on their own: one line with a shortcut, and the rich two-line context-ring tooltip. */
export const Bubbles: Story = {
  render: () => (
    <div className="flex items-start gap-24">
      <TooltipBubble label="Collapse sidebar" shortcut="Ctrl B" />
      <TooltipBubble label="Chat" />
      <TooltipBubble label="156k of 200k tokens" detail="claude-sonnet-5-5 · checkpoints automatically" />
    </div>
  ),
};
