import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { LoadMore } from "./LoadMore";

const meta = {
  title: "Components/Surfaces & data/LoadMore",
  component: LoadMore,
  tags: ["approved"],
  parameters: { vizier: { id: "X81", replaces: null } },
  args: { onLoad: () => {}, label: "Load older" },
  decorators: [(Story) => <div className="w-480">{Story()}</div>],
} satisfies Meta<typeof LoadMore>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const WithRemaining: Story = { args: { label: "Show all", remaining: 12 } };
export const Loading: Story = { args: { loading: true } };

/** Loads when scrolled into view. */
export const Auto: Story = {
  args: { look: "auto" },
  render: function Render(args) {
    const [n, setN] = useState(3);
    return (
      <div className="h-160 overflow-y-auto">
        {Array.from({ length: n }, (_, i) => (
          <p key={i} className="py-24 type-body text-muted">
            Entry {i + 1}
          </p>
        ))}
        <LoadMore {...args} onLoad={() => setN((v) => v + 3)} />
      </div>
    );
  },
};
