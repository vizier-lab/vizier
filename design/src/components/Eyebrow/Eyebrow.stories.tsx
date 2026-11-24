import type { Meta, StoryObj } from "@storybook/react-vite";
import { Eyebrow } from "./Eyebrow";

const meta = {
  title: "Components/Layout/Eyebrow",
  component: Eyebrow,
  tags: ["approved"],
  parameters: { vizier: { id: "X1", replaces: null } },
  args: { children: "Connected", count: 5 },
} satisfies Meta<typeof Eyebrow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Plain: Story = { args: { count: undefined, children: "Outline" } };

/** As drawn: counts, a range, an ordering, and a right-hand action. */
export const InUse: Story = {
  render: () => (
    <div className="flex w-480 flex-col gap-16">
      <Eyebrow count={5}>Connected</Eyebrow>
      <Eyebrow count="4 of 12" action={<span className="type-small text-muted">Show granted only</span>}>
        Permissions
      </Eyebrow>
      <Eyebrow count="A–Z">All</Eyebrow>
    </div>
  ),
};
