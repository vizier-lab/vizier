import type { Meta, StoryObj } from "@storybook/react-vite";
import { Mark } from "../Mark/Mark";
import { Wordmark } from "./Wordmark";

const meta = {
  title: "Brand/Wordmark",
  component: Wordmark,
  tags: ["approved"],
  parameters: {
    vizier: { id: "B2", replaces: null },
    docs: {
      description: {
        component:
          "Lowercase *vizier* in Schibsted Grotesk Bold with round i-dots. When live, the second i-dot turns emerald. In a lockup with the mark, only one of the two carries the live signal: the wordmark's i-dot.",
      },
    },
  },
  args: { state: "idle", size: 48 },
  argTypes: {
    state: { control: "inline-radio", options: ["idle", "live"] },
    size: { control: { type: "range", min: 14, max: 120, step: 2 } },
  },
} satisfies Meta<typeof Wordmark>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Live: Story = { args: { state: "live" } };

export const Sizes: Story = {
  render: (args) => (
    <div className="flex flex-col items-start gap-16">
      {[100, 48, 24, 16].map((size) => (
        <Wordmark key={size} {...args} size={size} />
      ))}
    </div>
  ),
};

/** Mark and wordmark together: the mark stays idle, the wordmark carries the signal. */
export const Lockup: Story = {
  args: { state: "live" },
  render: (args) => (
    <div className="flex items-center gap-12">
      <Mark size={40} title="" />
      <Wordmark {...args} size={40} />
    </div>
  ),
};
