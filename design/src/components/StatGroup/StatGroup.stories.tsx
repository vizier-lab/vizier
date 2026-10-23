import type { Meta, StoryObj } from "@storybook/react-vite";
import { StatGroup } from "./StatGroup";

const meta = {
  title: "Components/Layout/StatGroup",
  component: StatGroup,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X11", replaces: null } },
  args: {
    stats: [
      { label: "Estimated cost", value: "$18.42", sub: "+12% vs previous 30 days" },
      { label: "Tokens", value: "4.1M", sub: "3.2M in · 0.9M out" },
      { label: "Calls", value: "2,184", sub: "73 a day" },
      { label: "Turns", value: "611", sub: "median 3.4 calls" },
      { label: "Failed turns", value: "7", sub: "1.1% of turns", tone: "failed" },
    ],
  },
  decorators: [(Story) => <div className="w-1122">{Story()}</div>],
} satisfies Meta<typeof StatGroup>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The Usage summary strip. */
export const Default: Story = {};
