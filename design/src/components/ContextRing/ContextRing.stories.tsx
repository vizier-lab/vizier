import type { Meta, StoryObj } from "@storybook/react-vite";
import { TooltipBubble } from "../Tooltip/Tooltip";
import { ContextRing } from "./ContextRing";

const meta = {
  title: "Components/Chat/ContextRing",
  component: ContextRing,
  tags: ["approved"],
  parameters: { vizier: { id: "X36", replaces: null } },
  args: { used: 156000, window: 200000, detail: "claude-sonnet-5-5 · checkpoints automatically" },
  decorators: [(Story) => <div className="p-48">{Story()}</div>],
} satisfies Meta<typeof ContextRing>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Hover or focus it. */
export const Default: Story = {};

/** Empty, filling, nearly full. */
export const Levels: Story = {
  render: (args) => (
    <div className="flex items-center gap-16">
      {[0, 24000, 100000, 156000, 196000].map((u) => (
        <ContextRing key={u} {...args} used={u} />
      ))}
    </div>
  ),
};

/** With its tooltip showing, as on hover. */
export const Hover: Story = {
  render: (args) => (
    <div className="flex flex-col items-center gap-6">
      <TooltipBubble label="156k of 200k tokens" detail={args.detail} />
      <ContextRing {...args} />
    </div>
  ),
};
